//! Transaction Manager requests (packet type 0x0E, MS-TDS 2.2.6.9): begin,
//! commit, roll back or save a transaction through the protocol itself
//! rather than a `BEGIN TRAN`/`COMMIT` T-SQL batch. Ported from upstream
//! Tiberius (tiberius-rs/tiberius 8338bb8 and 89cab5d, by Matthew Jackson).
//! Thanks to the Tiberius team.

use super::{AllHeaderTy, Encode, ALL_HEADERS_LEN_TX, ALL_HEADERS_LEN_TX_HEADER};
use bytes::{BufMut, BytesMut};
use std::borrow::Cow;

/// MS-TDS 2.2.6.9 request types (`TM_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
enum RequestType {
    Begin = 5,
    Commit = 7,
    Rollback = 8,
    Save = 9,
}

enum_into_int!(RequestType => u16);

/// The isolation level for [`Client::begin_transaction_with_isolation`].
///
/// [`Client::begin_transaction_with_isolation`]: crate::Client::begin_transaction_with_isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum IsolationLevel {
    /// The server's (or session's) current isolation level.
    #[default]
    Unspecified = 0x00,
    /// `READ UNCOMMITTED`.
    ReadUncommitted = 0x01,
    /// `READ COMMITTED`.
    ReadCommitted = 0x02,
    /// `REPEATABLE READ`.
    RepeatableRead = 0x03,
    /// `SERIALIZABLE`.
    Serializable = 0x04,
    /// `SNAPSHOT` (the database must allow snapshot isolation).
    Snapshot = 0x05,
}

enum_into_int!(IsolationLevel => u8);

#[derive(Debug, Clone)]
pub(crate) struct TransactionManagerRequest<'a> {
    transaction_desc: [u8; 8],
    ty: RequestType,
    isolation_level: IsolationLevel,
    name: Cow<'a, str>,
}

impl<'a> TransactionManagerRequest<'a> {
    pub(crate) fn begin(
        desc: [u8; 8],
        isolation_level: IsolationLevel,
        name: Cow<'a, str>,
    ) -> Self {
        Self {
            transaction_desc: desc,
            ty: RequestType::Begin,
            isolation_level,
            name,
        }
    }

    pub(crate) fn commit(desc: [u8; 8], name: Cow<'a, str>) -> Self {
        Self {
            transaction_desc: desc,
            ty: RequestType::Commit,
            isolation_level: IsolationLevel::Unspecified,
            name,
        }
    }

    pub(crate) fn rollback(desc: [u8; 8], name: Cow<'a, str>) -> Self {
        Self {
            transaction_desc: desc,
            ty: RequestType::Rollback,
            isolation_level: IsolationLevel::Unspecified,
            name,
        }
    }

    pub(crate) fn save(desc: [u8; 8], name: Cow<'a, str>) -> Self {
        Self {
            transaction_desc: desc,
            ty: RequestType::Save,
            isolation_level: IsolationLevel::Unspecified,
            name,
        }
    }
}

impl Encode<BytesMut> for TransactionManagerRequest<'_> {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u32_le(ALL_HEADERS_LEN_TX);
        dst.put_u32_le(ALL_HEADERS_LEN_TX_HEADER);
        dst.put_u16_le(u16::from(AllHeaderTy::TransactionDescriptor));
        dst.put_slice(&self.transaction_desc);
        dst.put_u32_le(1);

        dst.put_u16_le(u16::from(self.ty));

        if self.ty == RequestType::Begin {
            dst.put_u8(u8::from(self.isolation_level));
        }

        // The transaction (or savepoint) name: a one-byte length, then
        // UTF-16LE. Unlike an ordinary B_VARCHAR, SQL Server reads that length
        // as a *byte* count, and rejects a character count with error 3976
        // ("The transaction name has the odd length 3"), found by live
        // testing. Upstream sends the character count. So names are capped
        // at 127 characters; overflow is an error rather than a wrap.
        let bytes = self
            .name
            .encode_utf16()
            .count()
            .checked_mul(2)
            .and_then(|bytes| u8::try_from(bytes).ok())
            .ok_or_else(|| {
                crate::Error::Conversion(
                    "transaction name is longer than 127 UTF-16 code units".into(),
                )
            })?;
        dst.put_u8(bytes);
        for u in self.name.encode_utf16() {
            dst.put_u16_le(u);
        }

        // Commit/rollback carry fBeginXact; 0 = don't start a new transaction.
        if matches!(self.ty, RequestType::Commit | RequestType::Rollback) {
            dst.put_u8(0);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(req: TransactionManagerRequest<'_>) -> Vec<u8> {
        let mut buf = BytesMut::new();
        req.encode(&mut buf).unwrap();
        buf.to_vec()
    }

    fn headers(desc: [u8; 8]) -> Vec<u8> {
        let mut v = 22u32.to_le_bytes().to_vec();
        v.extend_from_slice(&18u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&desc);
        v.extend_from_slice(&1u32.to_le_bytes());
        v
    }

    #[test]
    fn request_bodies_match_ms_tds() {
        let d = [1, 2, 3, 4, 5, 6, 7, 8];

        let mut begin = headers(d);
        begin.extend_from_slice(&[5, 0, 4, 4, b't', 0, b'x', 0]);
        assert_eq!(
            encode(TransactionManagerRequest::begin(
                d,
                IsolationLevel::Serializable,
                "tx".into()
            )),
            begin
        );

        let mut commit = headers(d);
        commit.extend_from_slice(&[7, 0, 0, 0]);
        assert_eq!(
            encode(TransactionManagerRequest::commit(d, "".into())),
            commit
        );

        let mut rollback = headers(d);
        rollback.extend_from_slice(&[8, 0, 2, b's', 0, 0]);
        assert_eq!(
            encode(TransactionManagerRequest::rollback(d, "s".into())),
            rollback
        );

        let mut save = headers(d);
        save.extend_from_slice(&[9, 0, 2, b's', 0]);
        assert_eq!(encode(TransactionManagerRequest::save(d, "s".into())), save);
    }

    #[test]
    fn over_long_name_is_an_error() {
        let mut buf = BytesMut::new();
        let req = TransactionManagerRequest::save([0; 8], "x".repeat(128).into());
        assert!(req.encode(&mut buf).is_err());
    }
}
