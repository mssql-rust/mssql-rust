use super::{Packet, PacketCodec, PacketHeader, HEADER_BYTES};
use crate::Error;
use asynchronous_codec::Decoder;
use bytes::{Buf, BytesMut};
use tracing::{event, Level};

pub trait Decode<B: Buf> {
    fn decode(src: &mut B) -> crate::Result<Self>
    where
        Self: Sized;
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < HEADER_BYTES {
            src.reserve(HEADER_BYTES);
            return Ok(None);
        }

        let header = PacketHeader::decode(&mut BytesMut::from(&src[0..HEADER_BYTES]))?;
        let length = usize::from(header.length());

        if src.len() < length {
            src.reserve(length);
            return Ok(None);
        }

        event!(
            Level::TRACE,
            "Reading a {:?} ({} bytes)",
            header.r#type(),
            length,
        );

        let header = PacketHeader::decode(src)?;

        if length < HEADER_BYTES {
            return Err(Error::Protocol("Invalid packet length".into()));
        }

        let payload = src.split_to(length - HEADER_BYTES);

        Ok(Some(Packet::new(header, payload)))
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.decode(buf)? {
            Some(frame) => Ok(Some(frame)),
            None => {
                if buf.is_empty() {
                    Ok(None)
                } else {
                    Err(std::io::Error::other("bytes remaining on stream").into())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(length: u16, payload: &[u8]) -> BytesMut {
        let mut bytes = BytesMut::from(
            &[
                super::super::PacketType::TabularResult as u8,
                super::super::PacketStatus::EndOfMessage as u8,
                (length >> 8) as u8,
                length as u8,
                0,
                0,
                1,
                0,
            ][..],
        );
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn decode_waits_for_a_complete_header_and_payload() {
        let mut codec = PacketCodec;
        let mut short_header = BytesMut::from(&[4, 1, 0][..]);
        assert!(codec.decode(&mut short_header).unwrap().is_none());

        let mut short_payload = packet(12, &[1, 2]);
        assert!(codec.decode(&mut short_payload).unwrap().is_none());
    }

    #[test]
    fn decode_returns_one_packet_and_leaves_following_bytes() {
        let mut codec = PacketCodec;
        let mut source = packet(10, &[1, 2]);
        source.extend_from_slice(&packet(8, &[]));

        let decoded = codec.decode(&mut source).unwrap().unwrap();
        assert_eq!(&[1, 2], &decoded.payload[..]);
        assert_eq!(8, source.len());
    }

    #[test]
    fn decode_rejects_lengths_shorter_than_the_header() {
        let mut codec = PacketCodec;
        let mut source = packet(7, &[]);
        assert!(matches!(codec.decode(&mut source), Err(Error::Protocol(_))));
    }

    #[test]
    fn decode_eof_distinguishes_empty_truncated_and_complete_input() {
        let mut codec = PacketCodec;
        assert!(codec.decode_eof(&mut BytesMut::new()).unwrap().is_none());

        let mut truncated = BytesMut::from(&[4, 1][..]);
        assert!(matches!(
            codec.decode_eof(&mut truncated),
            Err(Error::Io { .. })
        ));

        let mut complete = packet(8, &[]);
        assert!(codec.decode_eof(&mut complete).unwrap().is_some());
    }
}
