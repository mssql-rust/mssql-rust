use crate::{error::Error, sql_read_bytes::SqlReadBytes, ColumnData};

pub(crate) async fn decode<R>(src: &mut R, type_len: usize) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let recv_len = src.read_u8().await? as usize;

    let res = match (recv_len, type_len) {
        (0, 1) => ColumnData::U8(None),
        (0, 2) => ColumnData::I16(None),
        (0, 4) => ColumnData::I32(None),
        (0, _) => ColumnData::I64(None),
        (1, _) => ColumnData::U8(Some(src.read_u8().await?)),
        (2, _) => ColumnData::I16(Some(src.read_i16_le().await?)),
        (4, _) => ColumnData::I32(Some(src.read_i32_le().await?)),
        (8, _) => ColumnData::I64(Some(src.read_i64_le().await?)),
        _ => {
            return Err(Error::Protocol(
                format!("intn: length of {} is invalid", recv_len).into(),
            ))
        }
    };

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::{BufMut, BytesMut};

    // recv_len is a server-controlled length byte read straight off the
    // wire; only 0/1/2/4/8 are valid, so any other value used to hit
    // `unimplemented!()` and panic on nothing malformed. See
    // prisma/tiberius#424.
    #[tokio::test]
    async fn decode_rejects_invalid_length_instead_of_panicking() {
        let mut buf = BytesMut::new();
        buf.put_u8(3); // not one of 0, 1, 2, 4, 8

        let result = decode(&mut buf.into_sql_read_bytes(), 4).await;
        assert!(matches!(result, Err(Error::Protocol(_))));
    }
}
