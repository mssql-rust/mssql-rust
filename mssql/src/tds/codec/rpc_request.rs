use super::{AllHeaderTy, Encode, ALL_HEADERS_LEN_TX};
use crate::{tds::codec::ColumnData, BytesMutWithTypeInfo, Result, TypeInfo};
use bytes::{BufMut, BytesMut};
use enumflags2::{bitflags, BitFlags};
use std::borrow::Cow;

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RpcStatus {
    ByRefValue = 1 << 0,
    DefaultValue = 1 << 1,
    // reserved
    Encrypted = 1 << 3,
}

#[bitflags]
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RpcOption {
    WithRecomp = 1 << 0,
    NoMeta = 1 << 1,
    ReuseMeta = 1 << 2,
}

#[derive(Debug)]
pub struct TokenRpcRequest<'a> {
    proc_id: RpcProcIdValue<'a>,
    flags: BitFlags<RpcOption>,
    params: Vec<RpcParam<'a>>,
    transaction_desc: [u8; 8],
}

impl<'a> TokenRpcRequest<'a> {
    pub fn new<I>(proc_id: I, params: Vec<RpcParam<'a>>, transaction_desc: [u8; 8]) -> Self
    where
        I: Into<RpcProcIdValue<'a>>,
    {
        Self {
            proc_id: proc_id.into(),
            flags: BitFlags::empty(),
            params,
            transaction_desc,
        }
    }
}

#[derive(Debug)]
pub struct RpcParam<'a> {
    pub name: Cow<'a, str>,
    pub flags: BitFlags<RpcStatus>,
    pub value: ColumnData<'a>,
    /// Overrides the type info this parameter is sent with, instead of
    /// letting `value`'s `Encode` impl pick a default (`String` values
    /// default to `NVarchar`). Used by
    /// `Config::send_string_parameters_as_unicode(false)` to send string
    /// parameters as `VarChar` instead.
    pub type_info: Option<TypeInfo>,
    /// A table-valued parameter: sent instead of `value` when set.
    pub(crate) table: Option<super::TableValue<'a>>,
}

/// 2.2.6.6 RPC Request
#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum RpcProcId {
    CursorOpen = 2,
    CursorFetch = 7,
    CursorClose = 9,
    ExecuteSQL = 10,
    Prepare = 11,
    Execute = 12,
    PrepExec = 13,
    Unprepare = 15,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RpcProcIdValue<'a> {
    Name(Cow<'a, str>),
    Id(RpcProcId),
}

impl<'a, S> From<S> for RpcProcIdValue<'a>
where
    S: Into<Cow<'a, str>>,
{
    fn from(s: S) -> Self {
        Self::Name(s.into())
    }
}

impl<'a> From<RpcProcId> for RpcProcIdValue<'a> {
    fn from(id: RpcProcId) -> Self {
        Self::Id(id)
    }
}

impl<'a> Encode<BytesMut> for TokenRpcRequest<'a> {
    fn encode(self, dst: &mut BytesMut) -> Result<()> {
        dst.put_u32_le(ALL_HEADERS_LEN_TX as u32);
        dst.put_u32_le(ALL_HEADERS_LEN_TX as u32 - 4);
        dst.put_u16_le(AllHeaderTy::TransactionDescriptor as u16);
        dst.put_slice(&self.transaction_desc);
        dst.put_u32_le(1);

        match self.proc_id {
            RpcProcIdValue::Id(ref id) => {
                let val = (0xffff_u32) | ((*id as u16) as u32) << 16;
                dst.put_u32_le(val);
            }
            // 2.2.6.5 RPCReqBatch: a named procedure call's `ProcName` is a
            // plain US_VARCHAR - a u16 character count (not byte count)
            // followed by the UTF-16LE name, unlike `Id`'s `0xFFFF`-marked
            // NameLenProcID form above. Real, on-the-wire target (prisma/
            // tiberius#328 flagged the RPC-by-name path this covers), not
            // just a completion for its own sake.
            RpcProcIdValue::Name(ref name) => {
                // `as u16` used to truncate a name past 65535 UTF-16 units,
                // desyncing the request (upstream Tiberius acaa4a4).
                let units = u16::try_from(name.encode_utf16().count()).map_err(|_| {
                    crate::Error::Conversion(
                        "RPC procedure name is longer than 65535 UTF-16 code units".into(),
                    )
                })?;
                dst.put_u16_le(units);

                for codepoint in name.encode_utf16() {
                    dst.put_u16_le(codepoint);
                }
            }
        }

        dst.put_u16_le(self.flags.bits());

        for param in self.params.into_iter() {
            param.encode(dst)?;
        }

        Ok(())
    }
}

impl<'a> Encode<BytesMut> for RpcParam<'a> {
    fn encode(self, dst: &mut BytesMut) -> Result<()> {
        // ParamName is a B_VARCHAR: a u8 count of UTF-16 code units. The old
        // `length += 1` on a u8 overflowed past 255 (a panic in debug builds,
        // a wrapped count and a desynced request in release). `ProcParam`
        // names are caller-supplied, so this is reachable. Unfixed upstream
        // too; found while porting acaa4a4.
        let length = u8::try_from(self.name.encode_utf16().count()).map_err(|_| {
            crate::Error::Conversion(
                format!(
                    "RPC parameter name is longer than 255 UTF-16 code units: {:?}",
                    self.name
                )
                .into(),
            )
        })?;

        dst.put_u8(length);

        for codepoint in self.name.encode_utf16() {
            dst.put_u16_le(codepoint);
        }

        dst.put_u8(self.flags.bits());

        if let Some(table) = self.table {
            return table.encode(dst);
        }

        // Unlike bulk_insert (which declares column types once, up front, in
        // a COLMETADATA token), an RPC parameter's TYPE_INFO is inlined here
        // before its value on every call, so when a type override is given
        // we have to write that header ourselves -- ColumnData::encode's
        // Some(TypeInfo) arms only ever write the *value*, assuming the
        // header was already written by a bulk_insert-style caller.
        if let Some(ref ti) = self.type_info {
            ti.clone().encode(dst)?;
        }

        let mut dst_fi = BytesMutWithTypeInfo::new(dst);

        if let Some(ref ti) = self.type_info {
            dst_fi = dst_fi.with_type_info(ti);
        }

        self.value.encode(&mut dst_fi)?;

        Ok(())
    }
}

#[cfg(test)]
mod name_length_tests {
    use super::*;

    fn param(name: String) -> RpcParam<'static> {
        RpcParam {
            name: name.into(),
            flags: BitFlags::empty(),
            value: ColumnData::I32(Some(1)),
            type_info: None,
            table: None,
        }
    }

    #[test]
    fn over_long_param_name_errors_instead_of_wrapping() {
        let mut buf = BytesMut::new();
        assert!(param("p".repeat(256)).encode(&mut buf).is_err());

        let mut buf = BytesMut::new();
        param("p".repeat(255)).encode(&mut buf).unwrap();
        assert_eq!(buf[0], 255);
    }

    #[test]
    fn over_long_proc_name_errors_instead_of_truncating() {
        let req = TokenRpcRequest::new(
            RpcProcIdValue::Name("x".repeat(65_536).into()),
            Vec::new(),
            [0; 8],
        );
        let mut buf = BytesMut::new();
        assert!(req.encode(&mut buf).is_err());
    }
}
