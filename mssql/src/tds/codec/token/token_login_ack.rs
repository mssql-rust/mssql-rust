use crate::{Error, FeatureLevel, SqlReadBytes};
use std::convert::TryFrom;

#[allow(dead_code)] // we might want to debug the values
#[derive(Debug)]
pub struct TokenLoginAck {
    /// The type of interface with which the server will accept client requests
    /// 0: SQL_DFLT (server confirms that whatever is sent by the client is acceptable. If the client
    ///    requested SQL_DFLT, SQL_TSQL will be used)
    /// 1: SQL_TSQL (TSQL is accepted)
    pub(crate) interface: u8,
    pub(crate) tds_version: FeatureLevel,
    pub(crate) prog_name: String,
    /// major.minor.buildhigh.buildlow
    pub(crate) version: u32,
}

impl TokenLoginAck {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let mut body = super::TokenBody::read(src, "LOGINACK").await?;

        let interface = body.u8()?;

        let tds_version = FeatureLevel::try_from(body.u32_be()?)
            .map_err(|_| Error::Protocol("Login ACK: Invalid TDS version".into()))?;

        let prog_name = body.b_varchar()?;
        let version = body.u32_le()?;

        Ok(TokenLoginAck {
            interface,
            tds_version,
            prog_name,
            version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use bytes::{BufMut, BytesMut};

    fn login_ack(version: u32) -> BytesMut {
        let mut body = BytesMut::new();
        body.put_u8(1);
        body.put_u32(version);
        body.put_u8(3);
        for unit in "SQL".encode_utf16() {
            body.put_u16_le(unit);
        }
        body.put_u32_le(0x0102_0304);

        let mut wire = BytesMut::new();
        wire.put_u16_le(body.len() as u16);
        wire.extend_from_slice(&body);
        wire
    }

    #[tokio::test]
    async fn decodes_every_login_ack_field() {
        let mut source = login_ack(FeatureLevel::SqlServerN as u32).into_sql_read_bytes();
        let ack = TokenLoginAck::decode(&mut source).await.unwrap();
        assert_eq!(1, ack.interface);
        assert_eq!(FeatureLevel::SqlServerN, ack.tds_version);
        assert_eq!("SQL", ack.prog_name);
        assert_eq!(0x0102_0304, ack.version);
    }

    #[tokio::test]
    async fn rejects_unknown_tds_version() {
        let mut source = login_ack(0xdead_beef).into_sql_read_bytes();
        let error = TokenLoginAck::decode(&mut source).await.unwrap_err();
        assert!(matches!(error, Error::Protocol(_)), "{error:?}");
    }
}
