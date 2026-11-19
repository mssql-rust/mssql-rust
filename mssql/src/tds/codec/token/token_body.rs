//! Bounded parsing of length-prefixed token bodies.
//!
//! ERROR, INFO and LOGINACK tokens declare their body length up front, but
//! were decoded field by field straight off the stream, ignoring it. A
//! server whose declared length disagreed with the fields (a newer server
//! appending a field, or a hostile one) desynced every token after it. Now
//! the declared number of bytes is read first (at most 65535, so the
//! allocation is bounded), the fields are parsed out of that buffer, and the
//! stream is always left exactly at the next token. A body too short for its
//! fields is a protocol error; trailing extra bytes are ignored. Ported from
//! upstream Tiberius (tiberius-rs/tiberius bf96855, `decode_error_info_body`).

use crate::{Error, SqlReadBytes};
use futures_util::io::AsyncReadExt;

pub(crate) struct TokenBody {
    token: &'static str,
    buf: Vec<u8>,
    pos: usize,
}

impl TokenBody {
    /// Reads a u16 length and then exactly that many body bytes.
    pub(crate) async fn read<R>(src: &mut R, token: &'static str) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let len = src.read_u16_le().await? as usize;
        let mut buf = vec![0u8; len];
        src.read_exact(&mut buf).await?;
        Ok(Self { token, buf, pos: 0 })
    }

    fn take(&mut self, n: usize) -> crate::Result<&[u8]> {
        let end = self.pos.checked_add(n).filter(|&end| end <= self.buf.len());
        let Some(end) = end else {
            return Err(Error::Protocol(
                format!("{}: token body shorter than its fields", self.token).into(),
            ));
        };
        let bytes = &self.buf[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    pub(crate) fn u8(&mut self) -> crate::Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16_le(&mut self) -> crate::Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32_le(&mut self) -> crate::Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn u32_be(&mut self) -> crate::Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn utf16(&mut self, units: usize) -> crate::Result<String> {
        let token = self.token;
        let bytes = self.take(units * 2)?;
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16(&units)
            .map_err(|_| Error::Protocol(format!("{token}: invalid UTF-16 string").into()))
    }

    /// A B_VARCHAR: u8 count of UTF-16 code units, then the units.
    pub(crate) fn b_varchar(&mut self) -> crate::Result<String> {
        let units = self.u8()? as usize;
        self.utf16(units)
    }

    /// A US_VARCHAR: u16 count of UTF-16 code units, then the units.
    pub(crate) fn us_varchar(&mut self) -> crate::Result<String> {
        let units = self.u16_le()? as usize;
        self.utf16(units)
    }
}

#[cfg(test)]
mod tests {
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::tds::codec::{TokenError, TokenInfo, TokenOrder};
    use crate::SqlReadBytes;
    use bytes::{BufMut, BytesMut};

    fn utf16(dst: &mut Vec<u8>, s: &str) {
        s.encode_utf16()
            .for_each(|u| dst.extend_from_slice(&u.to_le_bytes()));
    }

    // ERROR/INFO body: number, state, class, US_VARCHAR msg, B_VARCHAR
    // server, B_VARCHAR proc, u32 line.
    fn error_info_body(msg: &str, extra: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&50000u32.to_le_bytes());
        b.extend_from_slice(&[1, 16]);
        b.extend_from_slice(&(msg.encode_utf16().count() as u16).to_le_bytes());
        utf16(&mut b, msg);
        b.push(3);
        utf16(&mut b, "srv");
        b.push(0);
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(extra);
        b
    }

    // A declared length longer than the known fields (e.g. a newer server
    // appending one) must not leave the extra bytes to be misread as the
    // next token.
    #[tokio::test]
    async fn trailing_bytes_in_error_and_info_bodies_are_skipped() {
        let body = error_info_body("boom", &[0xAA, 0xBB, 0xCC]);
        let mut buf = BytesMut::new();
        buf.put_u16_le(body.len() as u16);
        buf.put_slice(&body);
        buf.put_u16_le(body.len() as u16);
        buf.put_slice(&body);
        buf.put_u8(0x42); // sentinel where the next token would start

        let mut src = buf.into_sql_read_bytes();
        let err = TokenError::decode(&mut src).await.unwrap();
        assert_eq!((err.code(), err.message(), err.line()), (50000, "boom", 7));
        let info = TokenInfo::decode(&mut src).await.unwrap();
        assert_eq!(info.message, "boom");
        assert_eq!(src.read_u8().await.unwrap(), 0x42);
    }

    #[tokio::test]
    async fn body_shorter_than_its_fields_is_a_protocol_error() {
        let body = error_info_body("boom", &[]);
        let mut buf = BytesMut::new();
        buf.put_u16_le((body.len() - 3) as u16); // claims 3 bytes fewer
        buf.put_slice(&body[..body.len() - 3]);

        let err = TokenError::decode(&mut buf.into_sql_read_bytes())
            .await
            .unwrap_err();
        assert!(matches!(err, crate::Error::Protocol(_)), "{err:?}");
    }

    #[tokio::test]
    async fn odd_order_length_is_a_protocol_error() {
        let mut buf = BytesMut::new();
        buf.put_u16_le(3);
        buf.put_slice(&[1, 0, 2]);
        let err = TokenOrder::decode(&mut buf.into_sql_read_bytes())
            .await
            .unwrap_err();
        assert!(matches!(err, crate::Error::Protocol(_)), "{err:?}");
    }

    #[tokio::test]
    async fn invalid_utf16_in_a_token_body_is_a_protocol_error() {
        let mut body = Vec::new();
        body.extend_from_slice(&50000u32.to_le_bytes());
        body.extend_from_slice(&[1, 16]);
        body.extend_from_slice(&1u16.to_le_bytes());
        body.extend_from_slice(&0xd800u16.to_le_bytes());

        let mut wire = BytesMut::new();
        wire.put_u16_le(body.len() as u16);
        wire.put_slice(&body);
        let error = TokenError::decode(&mut wire.into_sql_read_bytes())
            .await
            .unwrap_err();
        assert!(matches!(error, crate::Error::Protocol(_)));
    }
}
