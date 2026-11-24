mod bytes_mut_with_data_columns;
mod into_row;
use crate::tds::codec::encode::Encode;
use crate::{tds::codec::ColumnData, BytesMutWithTypeInfo, Error, SqlReadBytes, TokenType};
use bytes::BufMut;
pub(crate) use bytes_mut_with_data_columns::BytesMutWithDataColumns;
use futures_util::io::AsyncReadExt;
pub use into_row::IntoRow;

/// A row of data.
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenRow<'a> {
    data: Vec<ColumnData<'a>>,
}

impl<'a> IntoIterator for TokenRow<'a> {
    type Item = ColumnData<'a>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a> Encode<BytesMutWithDataColumns<'a>> for TokenRow<'a> {
    fn encode(self, dst: &mut BytesMutWithDataColumns<'a>) -> crate::Result<()> {
        dst.put_u8(u8::from(TokenType::Row));

        if self.data.len() != dst.data_columns().len() {
            return Err(crate::Error::BulkInput(
                format!(
                    "Expecting {} columns but {} were given",
                    dst.data_columns().len(),
                    self.data.len()
                )
                .into(),
            ));
        }

        for (value, column) in self.data.into_iter().zip(dst.data_columns()) {
            let mut dst_ti = BytesMutWithTypeInfo::new(dst).with_type_info(&column.base.ty);
            value.encode(&mut dst_ti)?
        }

        Ok(())
    }
}

impl<'a> TokenRow<'a> {
    /// Creates a new empty row.
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Creates a new empty row with allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Clears the row, removing all column values.
    ///
    /// Note that this method has no effect on the allocated capacity of the row.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// The number of columns.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns an iterator over column values.
    pub fn iter(&self) -> std::slice::Iter<'_, ColumnData<'a>> {
        self.data.iter()
    }

    /// True if row has no columns.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Gets the columnar data with the given index. `None` if index out of
    /// bounds.
    pub fn get(&self, index: usize) -> Option<&ColumnData<'a>> {
        self.data.get(index)
    }

    /// Adds a new value to the row.
    pub fn push(&mut self, value: ColumnData<'a>) {
        self.data.push(value);
    }
}

impl TokenRow<'static> {
    /// Normal row. We'll read the metadata what we've cached and parse columns
    /// based on that.
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let Some(col_meta) = src.context().last_meta() else {
            return Err(Error::Protocol(
                "received a ROW token before any COLMETADATA".into(),
            ));
        };

        let mut row = Self {
            data: Vec::with_capacity(col_meta.columns.len()),
        };

        for column in col_meta.columns.iter() {
            let data = ColumnData::decode(src, &column.base.ty).await?;
            row.data.push(data);
        }

        Ok(row)
    }

    /// SQL Server has packed nulls on this row type. We'll read what columns
    /// are null from the bitmap.
    pub(crate) async fn decode_nbc<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let Some(col_meta) = src.context().last_meta() else {
            return Err(Error::Protocol(
                "received a ROW token before any COLMETADATA".into(),
            ));
        };
        let row_bitmap = RowBitmap::decode(src, col_meta.columns.len()).await?;

        let mut row = Self {
            data: Vec::with_capacity(col_meta.columns.len()),
        };

        for (i, column) in col_meta.columns.iter().enumerate() {
            let data = if row_bitmap.is_null(i) {
                column.base.null_value()
            } else {
                ColumnData::decode(src, &column.base.ty).await?
            };

            row.data.push(data);
        }

        Ok(row)
    }
}

#[cfg(test)]
mod accessor_tests {
    use super::TokenRow;
    use crate::ColumnData;

    #[test]
    fn capacity_iteration_lookup_clear_and_into_iter() {
        let mut row = TokenRow::with_capacity(2);
        assert!(row.is_empty());
        row.push(ColumnData::I32(Some(1)));
        row.push(ColumnData::I32(Some(2)));
        assert_eq!(2, row.len());
        assert_eq!(Some(&ColumnData::I32(Some(1))), row.get(0));
        assert_eq!(None, row.get(9));
        assert_eq!(2, row.iter().count());
        assert_eq!(2, row.clone().into_iter().count());
        row.clear();
        assert!(row.is_empty());
    }
}

/// A bitmap of null values in the row. Sometimes SQL Server decides to pack the
/// null values in the row, calling it the NBCROW. In this kind of tokens the row
/// itself skips the null columns completely, but they can be found from the bitmap
/// stored in the beginning of the token.
///
/// One byte can store eight bits of information. Bits with value of one being null.
///
/// If our row has eight columns, and our byte in bits is:
///
/// ```ignore
/// 1 0 0 1 0 1 0 0
/// ```
///
/// This would mean columns 0, 3 and 5 are null and should not be parsed at all.
/// For more than eight columns, more bits need to be reserved for the bitmap
/// (see the size calculation).
struct RowBitmap {
    data: Vec<u8>,
}

impl RowBitmap {
    /// Is the given column index null or not.
    #[inline]
    fn is_null(&self, i: usize) -> bool {
        let index = i / 8;
        let bit = i % 8;

        self.data[index] & (1 << bit) > 0
    }

    /// Decode the bitmap data from the beginning of the row. Only doable if the
    /// type is `NbcRowToken`.
    async fn decode<R>(src: &mut R, columns: usize) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let size = columns.div_ceil(8);
        let mut data = vec![0; size];
        src.read_exact(&mut data[0..size]).await?;

        Ok(Self { data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BaseMetaDataColumn, ColumnFlag, FixedLenType, MetaDataColumn, TypeInfo};
    use bytes::BytesMut;

    #[tokio::test]
    async fn wrong_number_of_columns_will_fail() {
        let row = (true, 5).into_row();
        let columns = vec![MetaDataColumn {
            base: BaseMetaDataColumn {
                flags: ColumnFlag::Nullable.into(),
                ty: TypeInfo::FixedLen(FixedLenType::Bit),
            },
            col_name: Default::default(),
        }];
        let mut buf = BytesMut::new();
        let mut buf_with_columns = BytesMutWithDataColumns::new(&mut buf, &columns);

        row.encode(&mut buf_with_columns)
            .expect_err("wrong number of columns");
    }

    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::tds::codec::token::token_col_metadata::TokenColMetaData;
    use crate::SqlReadBytes;
    use std::sync::Arc;

    fn int_column(name: &'static str) -> MetaDataColumn<'static> {
        MetaDataColumn {
            base: BaseMetaDataColumn {
                flags: ColumnFlag::Nullable.into(),
                ty: TypeInfo::FixedLen(FixedLenType::Int4),
            },
            col_name: name.into(),
        }
    }

    fn reader_with_two_int_columns(wire: &[u8]) -> impl SqlReadBytes {
        let mut reader = BytesMut::from(wire).into_sql_read_bytes();
        reader
            .context_mut()
            .set_last_meta(Arc::new(TokenColMetaData {
                columns: vec![int_column("a"), int_column("b")],
            }));
        reader
    }

    #[tokio::test]
    async fn a_row_before_any_column_metadata_is_a_protocol_error() {
        let expected = Error::Protocol("received a ROW token before any COLMETADATA".into());
        let mut reader = BytesMut::new().into_sql_read_bytes();
        assert_eq!(TokenRow::decode(&mut reader).await.unwrap_err(), expected);
        assert_eq!(
            TokenRow::decode_nbc(&mut reader).await.unwrap_err(),
            expected
        );
    }

    #[tokio::test]
    async fn rows_decode_against_the_cached_metadata() {
        let mut reader = reader_with_two_int_columns(&[1, 0, 0, 0, 2, 0, 0, 0]);
        let row = TokenRow::decode(&mut reader).await.unwrap();
        assert_eq!(
            row.data,
            vec![ColumnData::I32(Some(1)), ColumnData::I32(Some(2))]
        );

        // A truncated row surfaces the read error rather than a short row.
        let mut reader = reader_with_two_int_columns(&[1, 0, 0, 0]);
        assert!(TokenRow::decode(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn null_bitmap_rows_skip_the_columns_the_bitmap_marks() {
        // Bit 0 set: column `a` is NULL and absent from the wire.
        let mut reader = reader_with_two_int_columns(&[0b01, 9, 0, 0, 0]);
        let row = TokenRow::decode_nbc(&mut reader).await.unwrap();
        assert_eq!(
            row.data,
            vec![ColumnData::I32(None), ColumnData::I32(Some(9))]
        );

        // No bit set: both columns are present.
        let mut reader = reader_with_two_int_columns(&[0, 1, 0, 0, 0, 2, 0, 0, 0]);
        let row = TokenRow::decode_nbc(&mut reader).await.unwrap();
        assert_eq!(
            row.data,
            vec![ColumnData::I32(Some(1)), ColumnData::I32(Some(2))]
        );

        // A missing bitmap is a read error.
        let mut reader = reader_with_two_int_columns(&[]);
        assert!(TokenRow::decode_nbc(&mut reader).await.is_err());
    }

    #[test]
    fn a_row_encodes_as_the_row_token_followed_by_each_column() {
        let columns = vec![int_column("a")];
        let mut buf = BytesMut::new();
        let mut buf_with_columns = BytesMutWithDataColumns::new(&mut buf, &columns);

        5i32.into_row().encode(&mut buf_with_columns).unwrap();

        // The ROW token, then the fixed-width INT4 value.
        assert_eq!(buf[0], u8::from(TokenType::Row));
        assert_eq!(&buf[buf.len() - 4..], &5i32.to_le_bytes());
    }
}
