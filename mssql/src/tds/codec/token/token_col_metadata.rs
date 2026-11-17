use std::{
    borrow::{BorrowMut, Cow},
    fmt::Display,
};

use crate::{
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
                // A type `INSERT BULK` can't declare. This used to be
                // `unreachable!()`: a server-controlled panic, since
                // `TypeInfo::decode` accepts it. (Upstream Tiberius 3e6afc3.)
                FixedLenType::Null => return Err(std::fmt::Error),
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
                VarLenType::SSVariant => write!(f, "sql_variant")?,
                VarLenType::Intn => match ctx.len() {
                    1 => write!(f, "tinyint")?,
                    2 => write!(f, "smallint")?,
                    4 => write!(f, "int")?,
                    8 => write!(f, "bigint")?,
                    _ => return Err(std::fmt::Error),
                },
                VarLenType::Floatn => match ctx.len() {
                    4 => write!(f, "real")?,
                    8 => write!(f, "float")?,
                    _ => return Err(std::fmt::Error),
                },
                VarLenType::Money => match ctx.len() {
                    4 => write!(f, "smallmoney")?,
                    8 => write!(f, "money")?,
                    _ => return Err(std::fmt::Error),
                },
                _ => return Err(std::fmt::Error),
            },
            TypeInfo::VarLenSizedPrecision {
                ty,
                size: _,
                precision,
                scale,
            } => match ty {
                VarLenType::Decimaln => write!(f, "decimal({},{})", precision, scale)?,
                VarLenType::Numericn => write!(f, "numeric({},{})", precision, scale)?,
                _ => return Err(std::fmt::Error),
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
                VarLenType::Udt => ColumnData::Binary(None),
                VarLenType::Text => ColumnData::String(None),
                VarLenType::Image => ColumnData::Binary(None),
                VarLenType::NText => ColumnData::String(None),
                VarLenType::SSVariant => ColumnData::String(None),
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
                VarLenType::Udt => ColumnData::Binary(None),
                VarLenType::Text => ColumnData::String(None),
                VarLenType::Image => ColumnData::Binary(None),
                VarLenType::NText => ColumnData::String(None),
                VarLenType::SSVariant => ColumnData::String(None),
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

/// Encodes the COLMETADATA token this client sends ahead of bulk-load rows.
///
/// Same as `TokenColMetaData`'s `Encode`, except that `text`/`ntext`/`image`
/// columns also carry the destination table name after their TYPE_INFO. In
/// this client->server direction it is a single bare US_VARCHAR holding the
/// whole name: no NumParts byte, and no splitting on `.`. That differs from
/// the server->client shape the decoder reads (NumParts + parts). Omitting
/// it, or sending the result-style NumParts, makes the server misparse the
/// column and fail with "premature end of message" (error 4804), which is
/// why bulk-inserting into, or merely into a table *with*, a TEXT/NTEXT/IMAGE
/// column used to fail (fork issue #352). Ported from upstream Tiberius
/// (tiberius-rs/tiberius 93837ef, 103e044 and 74c5fa3), whose format was
/// checked against go-mssqldb's bulk copy. Kept as a separate function so
/// `BaseMetaDataColumn`'s public fields don't change.
pub(crate) fn encode_bulk_col_metadata(
    columns: &[MetaDataColumn<'_>],
    table_name: &str,
    dst: &mut BytesMut,
) -> crate::Result<()> {
    let count = u16::try_from(columns.len())
        .map_err(|_| crate::Error::BulkInput("too many columns for a bulk insert".into()))?;

    dst.put_u8(TokenType::ColMetaData as u8);
    dst.put_u16_le(count);

    for col in columns {
        dst.put_u32_le(0); // user type
        dst.put_u16_le(BitFlags::bits(col.base.flags));

        let carries_table_name = matches!(
            &col.base.ty,
            TypeInfo::VarLenSized(cx)
                if matches!(cx.r#type(), VarLenType::Text | VarLenType::NText | VarLenType::Image)
        );

        col.base.ty.clone().encode(dst)?;

        if carries_table_name {
            encode_us_varchar(dst, table_name)?;
        }

        encode_b_varchar(dst, &col.col_name)?;
    }

    Ok(())
}

/// A US_VARCHAR: a u16 count of UTF-16 code units, then UTF-16LE.
fn encode_us_varchar(dst: &mut BytesMut, s: &str) -> crate::Result<()> {
    let units = u16::try_from(s.encode_utf16().count()).map_err(|_| {
        crate::Error::BulkInput(format!("table name is too long for COLMETADATA: {s:?}").into())
    })?;
    dst.put_u16_le(units);
    for chr in s.encode_utf16() {
        dst.put_u16_le(chr);
    }
    Ok(())
}

/// A B_VARCHAR: a u8 count of UTF-16 code units, then UTF-16LE. The old
/// inline encoder counted in a `u8` that wrapped past 255 and desynced.
fn encode_b_varchar(dst: &mut BytesMut, s: &str) -> crate::Result<()> {
    let units = u8::try_from(s.encode_utf16().count()).map_err(|_| {
        crate::Error::BulkInput(format!("column name is too long for COLMETADATA: {s:?}").into())
    })?;
    dst.put_u8(units);
    for chr in s.encode_utf16() {
        dst.put_u16_le(chr);
    }
    Ok(())
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

        // MS-TDS marks several flag bits reserved ("must be ignored"), so a
        // newer server setting one must not fail the whole result set.
        // Ported from upstream Tiberius e59a57f.
        let flags = BitFlags::from_bits_truncate(src.read_u16_le().await?);

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

    fn us_varchar(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut out = (units.len() as u16).to_le_bytes().to_vec();
        units
            .iter()
            .for_each(|u| out.extend_from_slice(&u.to_le_bytes()));
        out
    }

    fn bulk_column(name: &str, ty: TypeInfo) -> MetaDataColumn<'static> {
        MetaDataColumn {
            base: BaseMetaDataColumn {
                flags: BitFlags::empty(),
                ty,
            },
            col_name: name.to_string().into(),
        }
    }

    // In the client->server bulk COLMETADATA a TEXT column's TYPE_INFO is
    // followed by the whole table name as one bare US_VARCHAR (no NumParts
    // byte, no splitting on `.`), then the column name. Other types carry
    // nothing extra. Ported from upstream Tiberius 103e044.
    #[test]
    fn bulk_col_metadata_puts_bare_table_name_after_text_type_info() {
        use crate::tds::codec::VarLenContext;

        let text_ty =
            TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Text, 0x7fff_ffff, None));
        let int_ty = TypeInfo::FixedLen(FixedLenType::Int4);

        let mut buf = BytesMut::new();
        encode_bulk_col_metadata(
            &[
                bulk_column("t", text_ty.clone()),
                bulk_column("i", int_ty.clone()),
            ],
            "dbo.MyTable",
            &mut buf,
        )
        .unwrap();

        let mut expected = vec![TokenType::ColMetaData as u8, 2, 0];
        for (name, ty, table) in [("t", text_ty, true), ("i", int_ty, false)] {
            expected.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // user type + flags
            let mut ti = BytesMut::new();
            ty.encode(&mut ti).unwrap();
            expected.extend_from_slice(&ti);
            if table {
                expected.extend(us_varchar("dbo.MyTable"));
            }
            expected.push(1);
            expected.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        }

        assert_eq!(buf.as_ref(), expected.as_slice());
    }

    #[test]
    fn bulk_col_metadata_rejects_over_long_names() {
        let ty = TypeInfo::FixedLen(FixedLenType::Int4);
        let long_col = "c".repeat(256);
        let mut buf = BytesMut::new();
        assert!(encode_bulk_col_metadata(&[bulk_column(&long_col, ty)], "t", &mut buf).is_err());

        use crate::tds::codec::VarLenContext;
        let text_ty =
            TypeInfo::VarLenSized(VarLenContext::new(VarLenType::NText, 0x7fff_ffff, None));
        let long_table = "t".repeat(u16::MAX as usize + 1);
        let mut buf = BytesMut::new();
        assert!(
            encode_bulk_col_metadata(&[bulk_column("c", text_ty)], &long_table, &mut buf).is_err()
        );
    }

    // Types INSERT BULK can't declare used to hit `unreachable!()` in Display
    // (a server-controlled panic); they now return `fmt::Error`, which the
    // bulk-insert path maps to Error::BulkInput.
    #[test]
    fn display_errors_instead_of_panicking_on_undeclarable_types() {
        use crate::tds::codec::VarLenContext;
        use std::fmt::Write;

        for ty in [
            TypeInfo::FixedLen(FixedLenType::Null),
            TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Intn, 3, None)),
            TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Floatn, 5, None)),
            TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Money, 2, None)),
        ] {
            let col = MetaDataColumn {
                base: BaseMetaDataColumn {
                    flags: BitFlags::empty(),
                    ty,
                },
                col_name: "c".into(),
            };
            let mut s = String::new();
            assert!(write!(s, "{col}").is_err());
        }
    }

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
