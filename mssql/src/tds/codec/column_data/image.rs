use crate::{sql_read_bytes::SqlReadBytes, ColumnData};

pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let ptr_len = usize::from(src.read_u8().await?);

    if ptr_len == 0 {
        return Ok(ColumnData::Binary(None));
    }

    for _ in 0..ptr_len {
        src.read_u8().await?;
    }

    src.read_i32_le().await?; // days
    src.read_u32_le().await?; // second fractions

    // `len` is an untrusted u32: read in bounded windows instead of
    // reserving up to 4 GiB up front (and one byte per poll).
    let len = crate::num::convert_wire(src.read_u32_le().await?, "image_length")?;
    let mut buf = Vec::new();
    crate::sql_read_bytes::read_bytes_into(src, &mut buf, len).await?;

    Ok(ColumnData::Binary(Some(buf.into())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::{BufMut, BytesMut};

    #[tokio::test]
    async fn image_values_decode_with_and_without_a_text_pointer() {
        let mut input = BytesMut::from(&[0u8][..]).into_sql_read_bytes();
        assert_eq!(decode(&mut input).await.unwrap(), ColumnData::Binary(None));

        let mut wire = BytesMut::new();
        wire.put_u8(2); // text pointer length
        wire.extend_from_slice(&[9, 9]); // text pointer
        wire.put_i32_le(0); // days
        wire.put_u32_le(0); // second fractions
        wire.put_u32_le(3);
        wire.extend_from_slice(&[1, 2, 3]);
        let mut input = wire.into_sql_read_bytes();
        assert_eq!(
            decode(&mut input).await.unwrap(),
            ColumnData::Binary(Some(vec![1, 2, 3].into()))
        );
    }
}
