use crate::{sql_read_bytes::SqlReadBytes, tds::codec::Encode};
use bytes::BytesMut;
use futures_util::io::AsyncReadExt;

#[derive(Debug)]
pub struct TokenSspi(Vec<u8>);

impl AsRef<[u8]> for TokenSspi {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TokenSspi {
    #[cfg(any(
        windows,
        all(unix, any(feature = "integrated-auth-gssapi", feature = "sspi-rs"))
    ))]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub(crate) async fn decode_async<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let len = usize::from(src.read_u16_le().await?);
        let mut bytes = vec![0; len];
        src.read_exact(&mut bytes[0..len]).await?;

        Ok(Self(bytes))
    }
}

impl Encode<BytesMut> for TokenSspi {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.extend(self.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;

    #[tokio::test]
    async fn sspi_token_decodes_and_encodes_its_payload() {
        let mut input = BytesMut::from(&[3, 0, 7, 8, 9][..]).into_sql_read_bytes();
        let token = TokenSspi::decode_async(&mut input).await.unwrap();
        assert_eq!(token.as_ref(), &[7, 8, 9]);

        let mut out = BytesMut::new();
        token.encode(&mut out).unwrap();
        assert_eq!(out.as_ref(), &[7, 8, 9]);

        let mut input = BytesMut::from(&[3, 0, 7][..]).into_sql_read_bytes();
        assert!(TokenSspi::decode_async(&mut input).await.is_err());
    }
}
