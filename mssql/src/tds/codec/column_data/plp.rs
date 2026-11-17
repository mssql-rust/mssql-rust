use crate::sql_read_bytes::SqlReadBytes;

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
                    let mut data = Vec::new();
                    crate::sql_read_bytes::read_bytes_into(src, &mut data, len as usize).await?;

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
                // Known size. `len` is an untrusted 64-bit wire value: cap the
                // up-front reservation (a value near u64::MAX used to be a
                // capacity-overflow panic, and anything large an OOM abort).
                _ => Vec::with_capacity((len as usize).min(crate::sql_read_bytes::MAX_PREALLOC)),
            };

            loop {
                let chunk_size = src.read_u32_le().await? as usize;

                if chunk_size == 0 {
                    break; // found a sentinel, we're done
                }

                // The chunk count of an "unknown length" PLP value is unbounded
                // on the wire, so cap the running total.
                if data.len().saturating_add(chunk_size) > super::MAX_PLP_SIZE {
                    return Err(crate::Error::Protocol(
                        format!(
                            "PLP value exceeds the maximum supported size of {} bytes",
                            super::MAX_PLP_SIZE
                        )
                        .into(),
                    ));
                }

                // Read the chunk in bulk rather than one byte at a time (see
                // the comment above), but grow the buffer only as bytes
                // arrive: `chunk_size` is an untrusted u32, and resizing to it
                // up front zero-filled up to 4 GiB before any data was read.
                crate::sql_read_bytes::read_bytes_into(src, &mut data, chunk_size).await?;
            }

            Ok(Some(data))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decode;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::{BufMut, BytesMut};

    // A known-length PLP header claiming ~u64::MAX bytes used to hit a
    // `Vec::with_capacity` capacity-overflow panic (or an OOM abort); a
    // u32::MAX chunk used to zero-fill 4 GiB before reading anything. Both
    // must now fail cleanly once the (missing) data runs out.
    #[tokio::test]
    async fn huge_declared_lengths_do_not_preallocate() {
        let mut buf = BytesMut::new();
        buf.put_u64_le(0xffff_ffff_ffff_fff0); // known total length
        buf.put_u32_le(u32::MAX); // first chunk
        buf.put_slice(b"abc"); // ...but only 3 bytes follow

        let res = decode(&mut buf.into_sql_read_bytes(), 0xffff).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn multi_chunk_value_reads_in_full() {
        let big = vec![7u8; 200_000]; // spans several MAX_PREALLOC windows
        let mut buf = BytesMut::new();
        buf.put_u64_le(0xffff_ffff_ffff_fffe); // unknown length
        buf.put_u32_le(big.len() as u32);
        buf.put_slice(&big);
        buf.put_u32_le(2);
        buf.put_slice(b"xy");
        buf.put_u32_le(0); // terminator

        let data = decode(&mut buf.into_sql_read_bytes(), 0xffff)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data.len(), big.len() + 2);
        assert_eq!(&data[big.len()..], b"xy");
    }
}
