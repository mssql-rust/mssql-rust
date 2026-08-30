use std::{
    borrow::{BorrowMut, Cow},
    fmt::Display,
};

use crate::{
    error::Error,
    tds::codec::{Encode, FixedLenType, TokenType, TypeInfo, VarLenType},
    Column, ColumnData, ColumnType, SqlReadBytes,
};
use asynchronous_codec::BytesMut;
use bytes::BufMut;
use enumflags2::{bitflags, BitFlags};

#[derive(Debug, Clone)]
pub struct TokenColMetaData<'a> {
    pub columns: Vec<MetaDataColumn<'a>>,
}

/// A single column's metadata, as returned by [`Client::column_metadata`]
/// and used internally to build a bulk insert's column list.
///
/// [`Client::column_metadata`]: crate::Client::column_metadata
#[derive(Debug, Clone)]
pub struct MetaDataColumn<'a> {
    /// The column's type and flags (nullability, identity, computed, etc).
    pub base: BaseMetaDataColumn,
    /// The column's name.
    pub col_name: Cow<'a, str>,
}

impl<'a> Display for MetaDataColumn<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Bracket-quote the column name so keyword-ish or space-containing
        // names (used to build INSERT BULK/SELECT TOP 0 statement text)
        // don't break the statement; a literal `]` inside the name must be
        // doubled, matching SQL Server's own escaping rule for quoted
        // identifiers.
        let col_name = self.col_name.replace(']', "]]");
        write!(f, "[{}] ", col_name)?;

        match &self.base.ty {
            TypeInfo::FixedLen(fixed) => match fixed {
                FixedLenType::Int1 => write!(f, "tinyint")?,
                FixedLenType::Bit => write!(f, "bit")?,
                FixedLenType::Int2 => write!(f, "smallint")?,
                FixedLenType::Int4 => write!(f, "int")?,
                FixedLenType::Datetime4 => write!(f, "smalldatetime")?,
                FixedLenType::Float4 => write!(f, "real")?,
                FixedLenType::Money => write!(f, "money")?,
                FixedLenType::Datetime => write!(f, "datetime")?,
                FixedLenType::Float8 => write!(f, "float")?,
                FixedLenType::Money4 => write!(f, "smallmoney")?,
                FixedLenType::Int8 => write!(f, "bigint")?,
                FixedLenType::Null => unreachable!(),
            },
            TypeInfo::VarLenSized(ctx) => match ctx.r#type() {
                VarLenType::Bitn => write!(f, "bit")?,
                VarLenType::Guid => write!(f, "uniqueidentifier")?,
                #[cfg(feature = "tds73")]
                VarLenType::Daten => write!(f, "date")?,
                #[cfg(feature = "tds73")]
                VarLenType::Timen => write!(f, "time")?,
                #[cfg(feature = "tds73")]
                VarLenType::Datetime2 => write!(f, "datetime2({})", ctx.len())?,
                VarLenType::Datetimen => write!(f, "datetime")?,
                #[cfg(feature = "tds73")]
                VarLenType::DatetimeOffsetn => write!(f, "datetimeoffset")?,
                VarLenType::BigVarBin => {
                    if ctx.len() <= 8000 {
                        write!(f, "varbinary({})", ctx.len())?
                    } else {
                        write!(f, "varbinary(max)")?
                    }
                }
                VarLenType::BigVarChar => {
                    if ctx.len() <= 8000 {
                        write!(f, "varchar({})", ctx.len())?
                    } else {
                        write!(f, "varchar(max)")?
                    }
                }
                VarLenType::BigBinary => write!(f, "binary({})", ctx.len())?,
                VarLenType::BigChar => write!(f, "char({})", ctx.len())?,
                VarLenType::NVarchar => {
                    if ctx.len() <= 4000 {
                        write!(f, "nvarchar({})", ctx.len())?
                    } else {
                        write!(f, "nvarchar(max)")?
                    }
                }
                VarLenType::NChar => write!(f, "nchar({})", ctx.len())?,
                VarLenType::Text => write!(f, "text")?,
                VarLenType::Image => write!(f, "image")?,
                VarLenType::NText => write!(f, "ntext")?,
                VarLenType::Intn => match ctx.len() {
                    1 => write!(f, "tinyint")?,
                    2 => write!(f, "smallint")?,
                    4 => write!(f, "int")?,
                    8 => write!(f, "bigint")?,
                    _ => unreachable!(),
                },
                VarLenType::Floatn => match ctx.len() {
                    4 => write!(f, "real")?,
                    8 => write!(f, "float")?,
                    _ => unreachable!(),
                },
                VarLenType::Money => match ctx.len() {
                    4 => write!(f, "smallmoney")?,
                    8 => write!(f, "money")?,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            TypeInfo::VarLenSizedPrecision {
                ty,
                size: _,
                precision,
                scale,
            } => match ty {
                VarLenType::Decimaln => write!(f, "decimal({},{})", precision, scale)?,
                VarLenType::Numericn => write!(f, "numeric({},{})", precision, scale)?,
                _ => unreachable!(),
            },
            TypeInfo::Xml { .. } => write!(f, "xml")?,
        }

        Ok(())
    }
}

/// The type and flags describing a column, shared between `COLMETADATA`
/// columns and bulk-insert column declarations.
#[derive(Debug, Clone)]
pub struct BaseMetaDataColumn {
    /// The column's attributes (nullability, identity, computed, etc.), as
    /// reported by the server.
    pub flags: BitFlags<ColumnFlag>,
    /// The column's wire type.
    pub ty: TypeInfo,
}

impl BaseMetaDataColumn {
    pub(crate) fn null_value(&self) -> ColumnData<'static> {
        match &self.ty {
            TypeInfo::FixedLen(ty) => match ty {
                FixedLenType::Null => ColumnData::I32(None),
                FixedLenType::Int1 => ColumnData::U8(None),
                FixedLenType::Bit => ColumnData::Bit(None),
                FixedLenType::Int2 => ColumnData::I16(None),
                FixedLenType::Int4 => ColumnData::I32(None),
                FixedLenType::Datetime4 => ColumnData::SmallDateTime(None),
                FixedLenType::Float4 => ColumnData::F32(None),
                FixedLenType::Money => ColumnData::F64(None),
                FixedLenType::Datetime => ColumnData::DateTime(None),
                FixedLenType::Float8 => ColumnData::F64(None),
                FixedLenType::Money4 => ColumnData::F32(None),
                FixedLenType::Int8 => ColumnData::I64(None),
            },
            TypeInfo::VarLenSized(cx) => match cx.r#type() {
                VarLenType::Guid => ColumnData::Guid(None),
                VarLenType::Intn => match cx.len() {
                    1 => ColumnData::U8(None),
                    2 => ColumnData::I16(None),
                    4 => ColumnData::I32(None),
                    _ => ColumnData::I64(None),
                },
                VarLenType::Bitn => ColumnData::Bit(None),
                VarLenType::Decimaln => ColumnData::Numeric(None),
                VarLenType::Numericn => ColumnData::Numeric(None),
                VarLenType::Floatn => match cx.len() {
                    4 => ColumnData::F32(None),
                    _ => ColumnData::F64(None),
                },
                VarLenType::Money => ColumnData::F64(None),
                VarLenType::Datetimen => ColumnData::DateTime(None),
                #[cfg(feature = "tds73")]
                VarLenType::Daten => ColumnData::Date(None),
                #[cfg(feature = "tds73")]
                VarLenType::Timen => ColumnData::Time(None),
                #[cfg(feature = "tds73")]
                VarLenType::Datetime2 => ColumnData::DateTime2(None),
                #[cfg(feature = "tds73")]
                VarLenType::DatetimeOffsetn => ColumnData::DateTimeOffset(None),
                VarLenType::BigVarBin => ColumnData::Binary(None),
                VarLenType::BigVarChar => ColumnData::String(None),
                VarLenType::BigBinary => ColumnData::Binary(None),
                VarLenType::BigChar => ColumnData::String(None),
                VarLenType::NVarchar => ColumnData::String(None),
                VarLenType::NChar => ColumnData::String(None),
                VarLenType::Xml => ColumnData::Xml(None),
                VarLenType::Udt => todo!("User-defined types not supported"),
                VarLenType::Text => ColumnData::String(None),
                VarLenType::Image => ColumnData::Binary(None),
                VarLenType::NText => ColumnData::String(None),
                VarLenType::SSVariant => todo!(),
            },
            TypeInfo::VarLenSizedPrecision { ty, .. } => match ty {
                VarLenType::Guid => ColumnData::Guid(None),
                VarLenType::Intn => ColumnData::I32(None),
                VarLenType::Bitn => ColumnData::Bit(None),
                VarLenType::Decimaln => ColumnData::Numeric(None),
                VarLenType::Numericn => ColumnData::Numeric(None),
                VarLenType::Floatn => ColumnData::F32(None),
                VarLenType::Money => ColumnData::F64(None),
                VarLenType::Datetimen => ColumnData::DateTime(None),
                #[cfg(feature = "tds73")]
                VarLenType::Daten => ColumnData::Date(None),
                #[cfg(feature = "tds73")]
                VarLenType::Timen => ColumnData::Time(None),
                #[cfg(feature = "tds73")]
                VarLenType::Datetime2 => ColumnData::DateTime2(None),
                #[cfg(feature = "tds73")]
                VarLenType::DatetimeOffsetn => ColumnData::DateTimeOffset(None),
                VarLenType::BigVarBin => ColumnData::Binary(None),
                VarLenType::BigVarChar => ColumnData::String(None),
                VarLenType::BigBinary => ColumnData::Binary(None),
                VarLenType::BigChar => ColumnData::String(None),
                VarLenType::NVarchar => ColumnData::String(None),
                VarLenType::NChar => ColumnData::String(None),
                VarLenType::Xml => ColumnData::Xml(None),
                VarLenType::Udt => todo!("User-defined types not supported"),
                VarLenType::Text => ColumnData::String(None),
                VarLenType::Image => ColumnData::Binary(None),
                VarLenType::NText => ColumnData::String(None),
                VarLenType::SSVariant => todo!(),
            },
            TypeInfo::Xml { .. } => ColumnData::Xml(None),
        }
    }
}

impl<'a> Encode<BytesMut> for TokenColMetaData<'a> {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u8(TokenType::ColMetaData as u8);
        dst.put_u16_le(self.columns.len() as u16);

        for col in self.columns.into_iter() {
            col.encode(dst)?;
        }

        Ok(())
    }
}

impl<'a> Encode<BytesMut> for MetaDataColumn<'a> {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u32_le(0);
        self.base.encode(dst)?;

        let len_pos = dst.len();
        let mut length = 0u8;

        dst.put_u8(length);

        for chr in self.col_name.encode_utf16() {
            length += 1;
            dst.put_u16_le(chr);
        }

        let dst: &mut [u8] = dst.borrow_mut();
        dst[len_pos] = length;

        Ok(())
    }
}

impl Encode<BytesMut> for BaseMetaDataColumn {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u16_le(BitFlags::bits(self.flags));
        self.ty.encode(dst)?;

        Ok(())
    }
}

/// A setting a column can hold.
///
/// Bit positions follow MS-TDS 2.2.7.4 COLMETADATA's `Flags` field exactly
/// (verified against
/// <https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/58880b9f-381c-43b2-bf8b-0727a98c4f4c>).
/// Note that `usUpdateable` is a single 2-bit *value* (0 = read-only, 1 =
/// read/write, 2 = updateable-unknown), not two independent flags, so
/// [`Updateable`](ColumnFlag::Updateable) and
/// [`UpdateableUnknown`](ColumnFlag::UpdateableUnknown) must be checked
/// together (e.g. via `intersects`) to mean "not definitely read-only" -
/// checking `Updateable` alone will miss columns the server reports as
/// "unknown" updateability, which in practice is most ordinary columns in a
/// plain `SELECT` (servers usually only resolve read-only vs. read/write
/// definitively for columns like identities or computed columns).
#[bitflags]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnFlag {
    /// The column can be null.
    Nullable = 1 << 0,
    /// Set for string columns with binary collation and always for the XML data
    /// type.
    CaseSensitive = 1 << 1,
    /// If column is writeable.
    Updateable = 1 << 2,
    /// Column modification status unknown.
    UpdateableUnknown = 1 << 3,
    /// Column is an identity.
    Identity = 1 << 4,
    /// Coulumn is computed.
    Computed = 1 << 5,
    /// Column is a fixed-length common language runtime user-defined type (CLR
    /// UDT).
    FixedLenClrType = 1 << 8,
    /// Column is the special XML column for the sparse column set.
    SparseColumnSet = 1 << 10,
    /// Column is encrypted transparently and has to be decrypted to view the
    /// plaintext value. This flag is valid when the column encryption feature
    /// is negotiated between client and server and is turned on.
    Encrypted = 1 << 11,
    /// Column is part of a hidden primary key created to support a T-SQL SELECT
    /// statement containing FOR BROWSE.
    Hidden = 1 << 13,
    /// Column is part of a primary key for the row and the T-SQL SELECT
    /// statement contains FOR BROWSE.
    Key = 1 << 14,
    /// It is unknown whether the column might be nullable.
    NullableUnknown = 1 << 15,
}

impl TokenColMetaData<'static> {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let column_count = src.read_u16_le().await?;
        let mut columns = Vec::with_capacity(column_count as usize);

        if column_count > 0 && column_count < 0xffff {
            for _ in 0..column_count {
                let base = BaseMetaDataColumn::decode(src).await?;
                let col_name = Cow::from(src.read_b_varchar().await?);

                columns.push(MetaDataColumn { base, col_name });
            }
        }

        Ok(TokenColMetaData { columns })
    }
}

impl<'a> TokenColMetaData<'a> {
    pub(crate) fn columns(&self) -> impl Iterator<Item = Column> + '_ {
        self.columns.iter().map(|x| Column {
            name: x.col_name.to_string(),
            column_type: ColumnType::from(&x.base.ty),
        })
    }
}

impl BaseMetaDataColumn {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        use VarLenType::*;

        let _user_ty = src.read_u32_le().await?;

        let flags = BitFlags::from_bits(src.read_u16_le().await?)
            .map_err(|_| Error::Protocol("column metadata: invalid flags".into()))?;

        let ty = TypeInfo::decode(src).await?;

        if let TypeInfo::VarLenSized(cx) = ty {
            if let Text | NText | Image = cx.r#type() {
                let num_of_parts = src.read_u8().await?;

                // table name
                for _ in 0..num_of_parts {
                    src.read_us_varchar().await?;
                }
            };
        };

        Ok(BaseMetaDataColumn { flags, ty })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test for the ColumnFlag bit positions, verified against
    // MS-TDS 2.2.7.4 COLMETADATA's `Flags` field:
    // https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/58880b9f-381c-43b2-bf8b-0727a98c4f4c
    //
    // Each assertion decodes a raw wire value containing exactly one real
    // flag bit (plus fNullable, always present for a nullable column so the
    // value stays realistic) and checks it lands on the right ColumnFlag.
    // Before this fix, several of these (Updateable, UpdateableUnknown,
    // Identity, Computed, FixedLenClrType, SparseColumnSet, Encrypted) were
    // off by 1-2 bits, so e.g. a real server's `fIdentity` bit (0x10) would
    // have been misread as `UpdateableUnknown`.
    fn flags(bits: u16) -> BitFlags<ColumnFlag> {
        BitFlags::from_bits(bits).unwrap()
    }

    #[test]
    fn nullable_bit_position() {
        assert!(flags(0x0001).contains(ColumnFlag::Nullable));
    }

    #[test]
    fn case_sensitive_bit_position() {
        assert!(flags(0x0002).contains(ColumnFlag::CaseSensitive));
    }

    #[test]
    fn updateable_bit_position() {
        // usUpdateable = 1 (read/write): bit 2 only.
        let f = flags(0x0004);
        assert!(f.contains(ColumnFlag::Updateable));
        assert!(!f.contains(ColumnFlag::UpdateableUnknown));
    }

    #[test]
    fn updateable_unknown_bit_position() {
        // usUpdateable = 2 (unknown): bit 3 only.
        let f = flags(0x0008);
        assert!(!f.contains(ColumnFlag::Updateable));
        assert!(f.contains(ColumnFlag::UpdateableUnknown));
    }

    #[test]
    fn identity_bit_position() {
        assert!(flags(0x0010).contains(ColumnFlag::Identity));
    }

    #[test]
    fn computed_bit_position() {
        assert!(flags(0x0020).contains(ColumnFlag::Computed));
    }

    #[test]
    fn fixed_len_clr_type_bit_position() {
        assert!(flags(0x0100).contains(ColumnFlag::FixedLenClrType));
    }

    #[test]
    fn sparse_column_set_bit_position() {
        assert!(flags(0x0400).contains(ColumnFlag::SparseColumnSet));
    }

    #[test]
    fn encrypted_bit_position() {
        assert!(flags(0x0800).contains(ColumnFlag::Encrypted));
    }

    #[test]
    fn hidden_bit_position() {
        assert!(flags(0x2000).contains(ColumnFlag::Hidden));
    }

    #[test]
    fn key_bit_position() {
        assert!(flags(0x4000).contains(ColumnFlag::Key));
    }

    #[test]
    fn nullable_unknown_bit_position() {
        assert!(flags(0x8000).contains(ColumnFlag::NullableUnknown));
    }
}
