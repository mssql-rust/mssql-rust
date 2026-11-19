use super::BaseMetaDataColumn;
use crate::{tds::codec::ColumnData, Error, SqlReadBytes};

#[derive(Debug)]
#[allow(dead_code)]
pub struct TokenReturnValue {
    pub param_ordinal: u16,
    pub param_name: String,
    /// return value of user defined function
    pub udf: bool,
    pub meta: BaseMetaDataColumn,
    pub value: ColumnData<'static>,
}

impl TokenReturnValue {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let param_ordinal = src.read_u16_le().await?;
        let param_name = src.read_b_varchar().await?;

        let udf = match src.read_u8().await? {
            0x01 => false,
            0x02 => true,
            _ => return Err(Error::Protocol("ReturnValue: invalid status".into())),
        };

        let meta = BaseMetaDataColumn::decode(src).await?;
        let value = ColumnData::decode(src, &meta.ty).await?;

        let token = TokenReturnValue {
            param_ordinal,
            param_name,
            udf,
            meta,
            value,
        };

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::TokenReturnValue;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::tds::codec::FixedLenType;
    use bytes::{BufMut, BytesMut};

    #[tokio::test]
    async fn rejects_unknown_parameter_status() {
        // ordinal 1, empty B_VARCHAR name, invalid status byte
        let mut source = BytesMut::from(&[1, 0, 0, 3][..]).into_sql_read_bytes();
        assert!(matches!(
            TokenReturnValue::decode(&mut source).await,
            Err(crate::Error::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn decodes_a_user_defined_function_return_value() {
        let mut wire = BytesMut::new();
        wire.put_u16_le(1); // ordinal
        wire.put_u8(0); // empty name
        wire.put_u8(2); // UDF return value
        wire.put_u32_le(0); // user type
        wire.put_u16_le(0); // flags
        wire.put_u8(FixedLenType::Int4 as u8);
        wire.put_i32_le(42);

        let value = TokenReturnValue::decode(&mut wire.into_sql_read_bytes())
            .await
            .unwrap();
        assert!(value.udf);
    }
}
