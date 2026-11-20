use super::{Decode, Encode, PacketHeader, PacketStatus, HEADER_BYTES};
use bytes::BytesMut;

#[derive(Debug)]
pub struct Packet {
    pub(crate) header: PacketHeader,
    pub(crate) payload: BytesMut,
}

impl Packet {
    pub(crate) fn new(header: PacketHeader, payload: BytesMut) -> Self {
        Self { header, payload }
    }

    pub(crate) fn is_last(&self) -> bool {
        self.header.status() == PacketStatus::EndOfMessage
    }

    pub(crate) fn into_parts(self) -> (PacketHeader, BytesMut) {
        (self.header, self.payload)
    }
}

impl Encode<BytesMut> for Packet {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        let size = (self.payload.len() as u16 + HEADER_BYTES as u16).to_be_bytes();

        self.header.encode(dst)?;
        // `BytesMut::extend` appends an iterator one byte at a time;
        // `extend_from_slice` copies the payload in one operation.
        dst.extend_from_slice(&self.payload);

        dst[2] = size[0];
        dst[3] = size[1];

        Ok(())
    }
}

impl Decode<BytesMut> for Packet {
    fn decode(src: &mut BytesMut) -> crate::Result<Self> {
        Ok(Self {
            header: PacketHeader::decode(src)?,
            payload: src.split(),
        })
    }
}

impl Extend<u8> for Packet {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.payload.extend(iter)
    }
}

impl<'a> Extend<&'a u8> for Packet {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.payload.extend(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_roundtrips_and_rewrites_wire_length() {
        let header = PacketHeader::login(4);
        let packet = Packet::new(header, BytesMut::from(&b"abc"[..]));
        assert!(packet.is_last());

        let mut wire = BytesMut::new();
        packet.encode(&mut wire).unwrap();
        assert_eq!(&[0, 11], &wire[2..4]);

        let decoded = Packet::decode(&mut wire).unwrap();
        let (header, payload) = decoded.into_parts();
        assert_eq!(PacketStatus::EndOfMessage, header.status());
        assert_eq!(11, header.length());
        assert_eq!(&b"abc"[..], &payload[..]);
    }

    #[test]
    fn packet_extends_from_owned_and_borrowed_bytes() {
        let mut packet = Packet::new(PacketHeader::batch(1), BytesMut::new());
        assert!(!packet.is_last());

        packet.extend([1_u8, 2]);
        packet.extend([3_u8, 4].iter());

        assert_eq!(&[1, 2, 3, 4], &packet.payload[..]);
    }

    #[test]
    fn encode_appends_large_packets_without_disturbing_existing_bytes() {
        let first: Vec<u8> = (0..=255).collect();
        let second = vec![0xab; 4_088];

        let mut wire = BytesMut::new();
        Packet::new(PacketHeader::batch(1), BytesMut::from(first.as_slice()))
            .encode(&mut wire)
            .unwrap();
        Packet::new(PacketHeader::batch(2), BytesMut::from(second.as_slice()))
            .encode(&mut wire)
            .unwrap();

        let first_len = HEADER_BYTES + first.len();
        assert_eq!(wire.len(), first_len + HEADER_BYTES + second.len());
        assert_eq!(&wire[HEADER_BYTES..first_len], first.as_slice());
        assert_eq!(&wire[first_len + HEADER_BYTES..], second.as_slice());
    }
}
