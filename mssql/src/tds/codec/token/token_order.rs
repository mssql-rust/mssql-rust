use crate::SqlReadBytes;

#[allow(dead_code)] // we might want to debug the values
#[derive(Debug)]
pub struct TokenOrder {
    pub(crate) column_indexes: Vec<u16>,
}

impl TokenOrder {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        // ORDER is a list of u16 column numbers; an odd byte length used to
        // be silently rounded down, leaving a stray byte that desynced the
        // stream (upstream Tiberius bf96855).
        let byte_len = src.read_u16_le().await?;
        if byte_len % 2 != 0 {
            return Err(crate::Error::Protocol(
                format!("ORDER: odd token length {byte_len}").into(),
            ));
        }
        let len = byte_len / 2;

        let mut column_indexes = Vec::with_capacity(len as usize);

        for _ in 0..len {
            column_indexes.push(src.read_u16_le().await?);
        }

        Ok(TokenOrder { column_indexes })
    }
}

#[cfg(test)]
mod tests {
    use super::TokenOrder;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::BytesMut;

    #[tokio::test]
    async fn decodes_empty_and_populated_column_lists() {
        let mut empty = BytesMut::from(&[0, 0][..]).into_sql_read_bytes();
        assert!(TokenOrder::decode(&mut empty)
            .await
            .unwrap()
            .column_indexes
            .is_empty());

        let mut populated =
            BytesMut::from(&[6, 0, 1, 0, 0xff, 0, 0xff, 0xff][..]).into_sql_read_bytes();
        assert_eq!(
            vec![1, 255, u16::MAX],
            TokenOrder::decode(&mut populated)
                .await
                .unwrap()
                .column_indexes
        );
    }
}
