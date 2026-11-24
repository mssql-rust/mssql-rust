use crate::{tds::codec::Encode, SqlReadBytes, TokenType};
use asynchronous_codec::BytesMut;
use bytes::BufMut;
use enumflags2::{bitflags, BitFlags};
use std::fmt;

#[derive(Debug, Default)]
pub struct TokenDone {
    status: BitFlags<DoneStatus>,
    cur_cmd: u16,
    done_rows: u64,
}

#[bitflags]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoneStatus {
    More = 1 << 0,
    Error = 1 << 1,
    Inexact = 1 << 2,
    // reserved
    Count = 1 << 4,
    Attention = 1 << 5,
    // reserved
    RpcInBatch = 1 << 7,
    SrvError = 1 << 8,
}

impl TokenDone {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        // Ignore reserved/unknown status bits rather than failing the stream
        // when a newer server sets one (ported from upstream Tiberius 3e6afc3).
        let status = BitFlags::from_bits_truncate(src.read_u16_le().await?);

        let cur_cmd = src.read_u16_le().await?;
        let done_row_count_bytes = src.context().version().done_row_count_bytes();

        let done_rows = match done_row_count_bytes {
            8 => src.read_u64_le().await?,
            4 => u64::from(src.read_u32_le().await?),
            other => {
                return Err(crate::Error::Protocol(
                    format!("DONE: unsupported row count width {other}").into(),
                ))
            }
        };

        Ok(TokenDone {
            status,
            cur_cmd,
            done_rows,
        })
    }

    pub(crate) fn is_final(&self) -> bool {
        self.status.is_empty()
    }

    pub(crate) fn rows(&self) -> u64 {
        self.done_rows
    }
}

impl Encode<BytesMut> for TokenDone {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u8(u8::from(TokenType::Done));
        dst.put_u16_le(BitFlags::bits(self.status));

        dst.put_u16_le(self.cur_cmd);
        dst.put_u64_le(self.done_rows);

        Ok(())
    }
}

impl fmt::Display for TokenDone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.done_rows == 0 {
            write!(f, "Done with status {:?}", self.status)
        } else if self.done_rows == 1 {
            write!(f, "Done with status {:?} (1 row left)", self.status)
        } else {
            write!(
                f,
                "Done with status {:?} ({} rows left)",
                self.status, self.done_rows
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::BytesMut;

    #[tokio::test]
    async fn decode_accessors_display_and_encode_cover_status_shapes() {
        let mut wire = BytesMut::new();
        wire.put_u16_le((DoneStatus::Count | DoneStatus::More).bits());
        wire.put_u16_le(7);
        wire.put_u64_le(2);

        let done = TokenDone::decode(&mut wire.into_sql_read_bytes())
            .await
            .unwrap();
        assert!(!done.is_final());
        assert_eq!(2, done.rows());
        assert!(done.to_string().contains("2 rows left"));

        let mut encoded = BytesMut::new();
        done.encode(&mut encoded).unwrap();
        assert_eq!(TokenType::Done as u8, encoded[0]);
        assert_eq!(13, encoded.len());

        assert_eq!(
            "Done with status BitFlags<DoneStatus>(0b0)",
            TokenDone::default().to_string()
        );
        let one = TokenDone {
            done_rows: 1,
            ..TokenDone::default()
        };
        assert!(one.to_string().contains("1 row left"));
    }

    #[tokio::test]
    async fn servers_before_2005_send_a_four_byte_row_count() {
        let mut wire = BytesMut::new();
        wire.put_u16_le(BitFlags::from(DoneStatus::Count).bits());
        wire.put_u16_le(7);
        wire.put_u32_le(9);

        let mut reader = wire.into_sql_read_bytes();
        reader
            .context_mut()
            .set_version(crate::tds::codec::FeatureLevel::SqlServer2000);
        let done = TokenDone::decode(&mut reader).await.unwrap();
        assert_eq!(9, done.rows());
    }
}
