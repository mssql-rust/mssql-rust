use crate::{Error, SqlReadBytes, FEA_EXT_FEDAUTH, FEA_EXT_TERMINATOR};
use futures_util::AsyncReadExt;

#[derive(Debug)]
pub struct TokenFeatureExtAck {
    pub features: Vec<FeatureAck>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum FedAuthAck {
    SecurityToken { nonce: Option<[u8; 32]> },
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum FeatureAck {
    FedAuth(FedAuthAck),
}

impl TokenFeatureExtAck {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let mut features = Vec::new();
        loop {
            let feature_id = src.read_u8().await?;

            if feature_id == FEA_EXT_TERMINATOR {
                break;
            } else if feature_id == FEA_EXT_FEDAUTH {
                let data_len = src.read_u32_le().await?;

                let nonce = if data_len == 32 {
                    let mut n = [0u8; 32];
                    src.read_exact(&mut n).await?;

                    Some(n)
                } else if data_len == 0 {
                    None
                } else {
                    return Err(Error::Protocol(
                        format!("invalid FEDAUTH data length: {}", data_len).into(),
                    ));
                };

                features.push(FeatureAck::FedAuth(FedAuthAck::SecurityToken { nonce }))
            } else {
                return Err(Error::Protocol(
                    format!("unsupported FEATUREEXTACK feature: {}", feature_id).into(),
                ));
            }
        }

        Ok(TokenFeatureExtAck { features })
    }
}

// Error-path tests ported from upstream Tiberius (tiberius-rs/tiberius
// 931223f); the success cases are fork additions.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::BytesMut;

    async fn decode(bytes: &[u8]) -> crate::Result<TokenFeatureExtAck> {
        TokenFeatureExtAck::decode(&mut BytesMut::from(bytes).into_sql_read_bytes()).await
    }

    #[tokio::test]
    async fn fedauth_ack_with_and_without_nonce() {
        let mut bytes = vec![FEA_EXT_FEDAUTH];
        bytes.extend_from_slice(&32u32.to_le_bytes());
        bytes.extend_from_slice(&[7u8; 32]);
        bytes.push(FEA_EXT_FEDAUTH);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.push(FEA_EXT_TERMINATOR);

        let ack = decode(&bytes).await.unwrap();
        assert_eq!(ack.features.len(), 2);
        let FeatureAck::FedAuth(FedAuthAck::SecurityToken { nonce }) = &ack.features[0];
        assert_eq!(*nonce, Some([7u8; 32]));
        let FeatureAck::FedAuth(FedAuthAck::SecurityToken { nonce }) = &ack.features[1];
        assert_eq!(*nonce, None);

        assert!(decode(&[FEA_EXT_TERMINATOR])
            .await
            .unwrap()
            .features
            .is_empty());
    }

    #[tokio::test]
    async fn invalid_fedauth_data_length_is_protocol_error() {
        let mut bytes = vec![FEA_EXT_FEDAUTH];
        bytes.extend_from_slice(&5u32.to_le_bytes());
        match decode(&bytes).await.unwrap_err() {
            Error::Protocol(msg) => assert!(msg.contains("invalid FEDAUTH data length"), "{msg}"),
            other => panic!("expected Error::Protocol, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unsupported_feature_id_is_protocol_error() {
        match decode(&[0x01]).await.unwrap_err() {
            Error::Protocol(msg) => {
                assert!(msg.contains("unsupported FEATUREEXTACK feature"), "{msg}")
            }
            other => panic!("expected Error::Protocol, got {other:?}"),
        }
    }
}
