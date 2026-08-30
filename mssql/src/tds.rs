pub mod codec;
mod collation;
mod context;
pub mod numeric;
pub mod stream;
pub mod time;
pub mod xml;

pub(crate) use collation::*;
pub(crate) use context::*;
pub(crate) use numeric::*;

/// The amount of bytes a packet header consists of
pub(crate) const HEADER_BYTES: usize = 8;

uint_enum! {
    /// The configured encryption level specifying if encryption is required
    #[repr(u8)]
    pub enum EncryptionLevel {
        /// Only use encryption for the login procedure
        Off = 0,
        /// Encrypt everything if possible
        On = 1,
        /// Do not encrypt anything
        NotSupported = 2,
        /// Encrypt everything and fail if not possible
        Required = 3,
        /// TDS 8.0 "strict" encryption (`Encrypt=Strict`): the TLS
        /// handshake happens before PRELOGIN rather than after it, closing
        /// a downgrade window the other levels leave open (a
        /// man-in-the-middle could otherwise tamper with the cleartext
        /// PRELOGIN exchange to make the client believe encryption wasn't
        /// available). Requires SQL Server 2022 or later, or Azure SQL
        /// Database/Managed Instance.
        Strict = 4,
    }

}

impl EncryptionLevel {
    /// The value actually placed on the wire in the PRELOGIN packet's
    /// `ENCRYPTION` field. `Strict`'s PRELOGIN exchange happens *inside* an
    /// already-established TLS session (see [`EncryptionLevel::Strict`]),
    /// so by the time this field is sent, the server has already committed
    /// to full encryption based on having received a TLS ClientHello before
    /// any TDS bytes at all - this field's value is documented as ignored
    /// by the server in that case, so `Required`'s value is sent, matching
    /// the closest real level TDS 7.x servers actually understand this
    /// PRELOGIN field to mean.
    pub(crate) fn as_wire_value(self) -> u8 {
        match self {
            EncryptionLevel::Strict => EncryptionLevel::Required as u8,
            other => other as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_level_as_wire_value() {
        assert_eq!(0, EncryptionLevel::Off.as_wire_value());
        assert_eq!(1, EncryptionLevel::On.as_wire_value());
        assert_eq!(2, EncryptionLevel::NotSupported.as_wire_value());
        assert_eq!(3, EncryptionLevel::Required.as_wire_value());
        assert_eq!(3, EncryptionLevel::Strict.as_wire_value());
    }
}
