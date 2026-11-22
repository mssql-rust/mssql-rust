use crate::{sql_read_bytes::SqlReadBytes, time::DateTimeOffset, ColumnData};

pub(crate) async fn decode<R>(src: &mut R, len: usize) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let rlen = src.read_u8().await?;

    let dto = match rlen {
        0 => ColumnData::DateTimeOffset(None),
        _ => {
            // The length byte covers the 5 bytes of date and offset.
            let time_len = rlen.checked_sub(5).ok_or_else(|| {
                crate::Error::Protocol(
                    "datetimeoffset: length is shorter than its date and offset".into(),
                )
            })?;
            let dto = DateTimeOffset::decode(src, len, time_len).await?;
            ColumnData::DateTimeOffset(Some(dto))
        }
    };

    Ok(dto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::BytesMut;

    #[tokio::test]
    async fn length_byte_shorter_than_its_date_and_offset_is_a_protocol_error() {
        for rlen in 1_u8..5 {
            let mut reader = BytesMut::from(&[rlen, 0, 0, 0, 0, 0, 0, 0][..]).into_sql_read_bytes();
            assert_eq!(
                decode(&mut reader, 7).await.unwrap_err(),
                crate::Error::Protocol(
                    "datetimeoffset: length is shorter than its date and offset".into()
                ),
                "length byte {rlen}"
            );
        }
    }

    #[tokio::test]
    async fn null_decodes_to_none() {
        let mut reader = BytesMut::from(&[0_u8][..]).into_sql_read_bytes();
        assert_eq!(
            decode(&mut reader, 0).await.unwrap(),
            ColumnData::DateTimeOffset(None)
        );
    }
}
