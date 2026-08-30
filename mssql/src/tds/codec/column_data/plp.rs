use crate::sql_read_bytes::SqlReadBytes;
use futures_util::io::AsyncReadExt;

// Decode a partially length-prefixed type.
pub(crate) async fn decode<R>(src: &mut R, len: usize) -> crate::Result<Option<Vec<u8>>>
where
    R: SqlReadBytes + Unpin,
{
    match len {
        // Fixed size
        len if len < 0xffff => {
            let len = src.read_u16_le().await? as u64;

            match len {
                // NULL
                0xffff => Ok(None),
                _ => {
                    // A single `read_exact` into a pre-sized buffer, instead
                    // of a `read_u8` loop pushing one byte at a time -
                    // reporters measured a 3x slowdown on varchar(max)-heavy
                    // workloads from the old per-byte version (#226).
                    let mut data = vec![0u8; len as usize];
                    src.read_exact(&mut data).await?;

                    Ok(Some(data))
                }
            }
        }
        // Unknown size, length-prefixed blobs
        _ => {
            let len = src.read_u64_le().await?;

            let mut data = match len {
                // NULL
                0xffffffffffffffff => return Ok(None),
                // Unknown size
                0xfffffffffffffffe => Vec::new(),
                // Known size
                _ => Vec::with_capacity(len as usize),
            };

            loop {
                let chunk_size = src.read_u32_le().await? as usize;

                if chunk_size == 0 {
                    break; // found a sentinel, we're done
                }

                // Read the whole chunk in one call rather than one byte at a
                // time; see the comment above for why this matters.
                let start = data.len();
                data.resize(start + chunk_size, 0);
                src.read_exact(&mut data[start..]).await?;
            }

            Ok(Some(data))
        }
    }
}
