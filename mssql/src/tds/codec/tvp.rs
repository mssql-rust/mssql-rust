//! Table-valued parameters (TVP_TYPE_INFO, MS-TDS 2.2.5.5.5). Adapted from
//! upstream Tiberius (tiberius-rs/tiberius 73e79d4's `type_info_tvp.rs`, by
//! Matthew Jackson), reworked without its derive macro. Thanks to the
//! Tiberius team.

use super::{
    BaseMetaDataColumn, BytesMutWithTypeInfo, Encode, FixedLenType, TypeInfo, VarLenContext,
    VarLenType,
};
use crate::TokenRow;
use bytes::{BufMut, BytesMut};

const TVP_TYPE: u8 = 0xF3;
const TVP_ROW_TOKEN: u8 = 0x01;
const TVP_END_TOKEN: u8 = 0x00;

/// A table-valued parameter ready to encode: the table type's (schema,
/// name), its columns as reported by the server, and the rows.
#[derive(Debug, Clone)]
pub(crate) struct TableValue<'a> {
    pub(crate) schema: String,
    pub(crate) type_name: String,
    pub(crate) columns: Vec<BaseMetaDataColumn>,
    pub(crate) rows: Vec<TokenRow<'a>>,
}

fn b_varchar(dst: &mut BytesMut, s: &str, what: &str) -> crate::Result<()> {
    let units = u8::try_from(s.encode_utf16().count()).map_err(|_| {
        crate::Error::Conversion(format!("{what} is longer than 255 UTF-16 code units").into())
    })?;
    dst.put_u8(units);
    s.encode_utf16().for_each(|u| dst.put_u16_le(u));
    Ok(())
}

/// TVP column metadata must use the nullable variable-length form of a
/// fixed-length type (MS-TDS 2.2.5.5.5.3), e.g. INTN for INT.
pub(crate) fn tvp_column_type(ty: TypeInfo) -> TypeInfo {
    let TypeInfo::FixedLen(fixed) = ty else {
        return ty;
    };
    let (vty, len) = match fixed {
        FixedLenType::Int1 => (VarLenType::Intn, 1),
        FixedLenType::Int2 => (VarLenType::Intn, 2),
        FixedLenType::Int4 => (VarLenType::Intn, 4),
        FixedLenType::Int8 => (VarLenType::Intn, 8),
        FixedLenType::Bit => (VarLenType::Bitn, 1),
        FixedLenType::Float4 => (VarLenType::Floatn, 4),
        FixedLenType::Float8 => (VarLenType::Floatn, 8),
        FixedLenType::Money4 => (VarLenType::Money, 4),
        FixedLenType::Money => (VarLenType::Money, 8),
        FixedLenType::Datetime4 => (VarLenType::Datetimen, 4),
        FixedLenType::Datetime => (VarLenType::Datetimen, 8),
        FixedLenType::Null => return TypeInfo::FixedLen(fixed),
    };
    TypeInfo::VarLenSized(VarLenContext::new(vty, len, None))
}

impl Encode<BytesMut> for TableValue<'_> {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u8(TVP_TYPE);
        b_varchar(dst, "", "TVP database name")?; // must be empty
        b_varchar(dst, &self.schema, "TVP schema name")?;
        b_varchar(dst, &self.type_name, "TVP type name")?;

        let count = u16::try_from(self.columns.len())
            .ok()
            .filter(|&n| n != 0xFFFF)
            .ok_or_else(|| crate::Error::Conversion("too many TVP columns".into()))?;
        dst.put_u16_le(count);

        for col in &self.columns {
            dst.put_u32_le(0); // UserType
            col.clone().encode(dst)?; // Flags + TYPE_INFO
            dst.put_u8(0); // ColName: must be empty in a TVP
        }

        dst.put_u8(TVP_END_TOKEN); // no optional metadata (order/unique)

        for (i, row) in self.rows.into_iter().enumerate() {
            if row.len() != self.columns.len() {
                return Err(crate::Error::Conversion(
                    format!(
                        "TVP row {i} has {} values, but the table type has {} columns",
                        row.len(),
                        self.columns.len()
                    )
                    .into(),
                ));
            }

            dst.put_u8(TVP_ROW_TOKEN);
            for (value, col) in row.into_iter().zip(&self.columns) {
                let mut dst_ti = BytesMutWithTypeInfo::new(dst).with_type_info(&col.ty);
                value.encode(&mut dst_ti)?;
            }
        }

        dst.put_u8(TVP_END_TOKEN);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tds::codec::ColumnFlag;
    use crate::ColumnData;
    use enumflags2::BitFlags;

    fn int_column() -> BaseMetaDataColumn {
        BaseMetaDataColumn {
            flags: BitFlags::from(ColumnFlag::Nullable),
            ty: tvp_column_type(TypeInfo::FixedLen(FixedLenType::Int4)),
        }
    }

    #[test]
    fn encodes_type_name_metadata_rows_and_end_token() {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(7)));
        let tvp = TableValue {
            schema: "dbo".into(),
            type_name: "T".into(),
            columns: vec![int_column()],
            rows: vec![row],
        };
        let mut buf = BytesMut::new();
        tvp.encode(&mut buf).unwrap();

        let mut expected = vec![TVP_TYPE, 0, 3, b'd', 0, b'b', 0, b'o', 0, 1, b'T', 0];
        expected.extend_from_slice(&1u16.to_le_bytes());
        expected.extend_from_slice(&[0, 0, 0, 0]); // user type
        expected.extend_from_slice(&(ColumnFlag::Nullable as u16).to_le_bytes());
        expected.extend_from_slice(&[VarLenType::Intn as u8, 4]); // TYPE_INFO
        expected.push(0); // empty column name
        expected.push(TVP_END_TOKEN);
        expected.extend_from_slice(&[TVP_ROW_TOKEN, 4, 7, 0, 0, 0]);
        expected.push(TVP_END_TOKEN);
        assert_eq!(buf.as_ref(), expected.as_slice());
    }

    #[test]
    fn a_row_with_the_wrong_width_is_an_error_not_a_panic() {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(1)));
        row.push(ColumnData::I32(Some(2)));
        let tvp = TableValue {
            schema: String::new(),
            type_name: "T".into(),
            columns: vec![int_column()],
            rows: vec![row],
        };
        assert!(tvp.encode(&mut BytesMut::new()).is_err());
    }

    #[test]
    fn every_fixed_type_uses_its_nullable_tvp_wire_type() {
        let cases = [
            (FixedLenType::Int1, Some((VarLenType::Intn, 1))),
            (FixedLenType::Int2, Some((VarLenType::Intn, 2))),
            (FixedLenType::Int4, Some((VarLenType::Intn, 4))),
            (FixedLenType::Int8, Some((VarLenType::Intn, 8))),
            (FixedLenType::Bit, Some((VarLenType::Bitn, 1))),
            (FixedLenType::Float4, Some((VarLenType::Floatn, 4))),
            (FixedLenType::Float8, Some((VarLenType::Floatn, 8))),
            (FixedLenType::Money4, Some((VarLenType::Money, 4))),
            (FixedLenType::Money, Some((VarLenType::Money, 8))),
            (FixedLenType::Datetime4, Some((VarLenType::Datetimen, 4))),
            (FixedLenType::Datetime, Some((VarLenType::Datetimen, 8))),
            (FixedLenType::Null, None),
        ];

        for (fixed, expected) in cases {
            let converted = tvp_column_type(TypeInfo::FixedLen(fixed));
            match expected {
                Some((ty, len)) => {
                    let TypeInfo::VarLenSized(context) = converted else {
                        panic!("{fixed:?} did not become variable length");
                    };
                    assert_eq!(ty, context.r#type());
                    assert_eq!(len, context.len());
                }
                None => assert!(matches!(converted, TypeInfo::FixedLen(FixedLenType::Null))),
            }
        }

        let variable = TypeInfo::VarLenSized(VarLenContext::new(VarLenType::NVarchar, 20, None));
        assert!(matches!(
            tvp_column_type(variable),
            TypeInfo::VarLenSized(_)
        ));
    }

    #[test]
    fn rejects_overlong_tvp_names() {
        let tvp = TableValue {
            schema: "s".repeat(256),
            type_name: "T".into(),
            columns: Vec::new(),
            rows: Vec::new(),
        };
        assert!(matches!(
            tvp.encode(&mut BytesMut::new()),
            Err(crate::Error::Conversion(_))
        ));
    }
}
