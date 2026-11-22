use crate::{sql_read_bytes::SqlReadBytes, time::DateTime2, ColumnData};

pub(crate) async fn decode<R>(src: &mut R, len: usize) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let rlen = src.read_u8().await?;

    let date = match rlen {
        0 => ColumnData::DateTime2(None),
        rlen => {
            // The length byte covers the 3-byte date, so it is at least 3.
            let time_len = usize::from(rlen).checked_sub(3).ok_or_else(|| {
                crate::Error::Protocol("datetime2: length is shorter than its date".into())
            })?;
            let dt = DateTime2::decode(src, len, time_len).await?;
            ColumnData::DateTime2(Some(dt))
        }
    };

    Ok(date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::BytesMut;

    #[tokio::test]
    async fn length_byte_shorter_than_the_date_is_a_protocol_error() {
        for rlen in [1_u8, 2] {
            let mut reader = BytesMut::from(&[rlen, 0, 0, 0, 0, 0, 0, 0][..]).into_sql_read_bytes();
            assert_eq!(
                decode(&mut reader, 7).await.unwrap_err(),
                crate::Error::Protocol("datetime2: length is shorter than its date".into()),
                "length byte {rlen}"
            );
        }
    }

    #[tokio::test]
    async fn null_and_minimum_length_decode() {
        let mut reader = BytesMut::from(&[0_u8][..]).into_sql_read_bytes();
        assert_eq!(
            decode(&mut reader, 0).await.unwrap(),
            ColumnData::DateTime2(None)
        );

        // 3-byte time (scale 0) + 3-byte date = 6 bytes.
        let mut reader = BytesMut::from(&[6_u8, 1, 0, 0, 2, 0, 0][..]).into_sql_read_bytes();
        let ColumnData::DateTime2(Some(dt2)) = decode(&mut reader, 0).await.unwrap() else {
            panic!("expected a datetime2");
        };
        assert_eq!(dt2.time().increments(), 1);
        assert_eq!(dt2.date().days(), 2);
    }
}
