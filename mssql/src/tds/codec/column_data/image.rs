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
