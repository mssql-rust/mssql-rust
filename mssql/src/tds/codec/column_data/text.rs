use crate::{error::Error, sql_read_bytes::SqlReadBytes, tds::Collation, ColumnData};

pub(crate) async fn decode<R>(
    src: &mut R,
    collation: Option<Collation>,
) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let ptr_len = src.read_u8().await? as usize;

    if ptr_len == 0 {
        return Ok(ColumnData::String(None));
    }

    for _ in 0..ptr_len {
        src.read_u8().await?;
    }

    src.read_i32_le().await?; // days
    src.read_u32_le().await?; // second fractions

    let text = match collation {
        // TEXT
        Some(collation) => {
            let codec = collation.codec()?;
            // `text_len` is an untrusted u32: read in bounded windows
            // instead of reserving up to 4 GiB up front.
            let text_len = src.read_u32_le().await? as usize;
            let mut buf = Vec::new();
            crate::sql_read_bytes::read_bytes_into(src, &mut buf, text_len).await?;

            codec.decode_with(buf.as_ref(), src.context().lossy_codepage_decoding())?
        }
        // NTEXT
        None => {
            let text_len = src.read_u32_le().await? as usize;
            if !text_len.is_multiple_of(2) {
                return Err(Error::Protocol("ntext: invalid length".into()));
            }

            let mut bytes = Vec::new();
            crate::sql_read_bytes::read_bytes_into(src, &mut bytes, text_len).await?;

            let buf: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();

            String::from_utf16_lossy(&buf[..])
        }
    };

    Ok(ColumnData::String(Some(text.into())))
}
