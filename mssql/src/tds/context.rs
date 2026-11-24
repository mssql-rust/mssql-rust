use super::codec::*;
use super::HEADER_BYTES;
use std::sync::Arc;

/// The TDS PacketID value after 255 (MS-TDS 2.2.3.1.4).
const PACKET_ID_RESTART: u8 = 0;

/// Context, that might be required to make sure we understand and are understood by the server
#[derive(Debug)]
pub(crate) struct Context {
    version: FeatureLevel,
    packet_size: u32,
    packet_id: u8,
    transaction_desc: [u8; 8],
    last_meta: Option<Arc<TokenColMetaData<'static>>>,
    spn: Option<String>,
    send_string_parameters_as_unicode: bool,
    lossy_codepage_decoding: bool,
    /// Per-round-trip deadline for reading command results; set from
    /// `Config::command_timeout` only after login, so it never bounds the
    /// handshake. `None` means unbounded.
    command_timeout: Option<std::time::Duration>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            version: FeatureLevel::SqlServerN,
            packet_size: 4096,
            packet_id: 0,
            transaction_desc: [0; 8],
            last_meta: None,
            spn: None,
            send_string_parameters_as_unicode: true,
            lossy_codepage_decoding: false,
            command_timeout: None,
        }
    }

    pub fn set_send_string_parameters_as_unicode(&mut self, enabled: bool) {
        self.send_string_parameters_as_unicode = enabled;
    }

    pub fn send_string_parameters_as_unicode(&self) -> bool {
        self.send_string_parameters_as_unicode
    }

    pub fn set_lossy_codepage_decoding(&mut self, enabled: bool) {
        self.lossy_codepage_decoding = enabled;
    }

    pub fn lossy_codepage_decoding(&self) -> bool {
        self.lossy_codepage_decoding
    }

    pub(crate) fn command_timeout(&self) -> Option<std::time::Duration> {
        self.command_timeout
    }

    pub(crate) fn set_command_timeout(&mut self, timeout: Option<std::time::Duration>) {
        self.command_timeout = timeout;
    }

    pub fn next_packet_id(&mut self) -> u8 {
        let id = self.packet_id;
        // Fallback: PACKET_ID_RESTART (0) - the one-byte TDS PacketID sequence
        // restarts at 0 after 255 (MS-TDS 2.2.3.1.4), so the first overflow is
        // the declared restart, not an error.
        self.packet_id = self.packet_id.checked_add(1).unwrap_or(PACKET_ID_RESTART);
        id
    }

    pub fn set_last_meta(&mut self, meta: Arc<TokenColMetaData<'static>>) {
        self.last_meta.replace(meta);
    }

    pub fn last_meta(&self) -> Option<Arc<TokenColMetaData<'static>>> {
        self.last_meta.clone()
    }

    #[cfg(test)]
    pub fn packet_size(&self) -> u32 {
        self.packet_size
    }

    /// The payload bytes one packet can carry: the negotiated packet size
    /// less the packet header. A negotiated size that leaves no room for a
    /// payload is invalid wire data (RCA-DECL-5) and is an error.
    pub(crate) fn payload_size(&self) -> crate::Result<usize> {
        usize::try_from(self.packet_size)
            .ok()
            .and_then(|size| size.checked_sub(HEADER_BYTES))
            .filter(|payload| *payload > 0)
            .ok_or_else(|| {
                crate::Error::Protocol("negotiated packet size leaves no room for a payload".into())
            })
    }

    pub fn set_packet_size(&mut self, new_size: u32) {
        self.packet_size = new_size;
    }

    pub fn transaction_descriptor(&self) -> [u8; 8] {
        self.transaction_desc
    }

    pub fn set_transaction_descriptor(&mut self, desc: [u8; 8]) {
        self.transaction_desc = desc;
    }

    pub fn version(&self) -> FeatureLevel {
        self.version
    }

    /// Lets a test decode as an older server would.
    #[cfg(test)]
    pub(crate) fn set_version(&mut self, version: FeatureLevel) {
        self.version = version;
    }

    pub fn set_spn(&mut self, host: impl AsRef<str>, port: u16) {
        self.spn = Some(format!("MSSQLSvc/{}:{}", host.as_ref(), port));
    }

    #[cfg(any(
        windows,
        all(unix, any(feature = "integrated-auth-gssapi", feature = "sspi-rs"))
    ))]
    pub fn spn(&self) -> &str {
        self.spn.as_deref().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn packet_id_counts_up_and_restarts_at_zero() {
        let mut context = Context::new();
        assert_eq!(context.next_packet_id(), 0);
        assert_eq!(context.next_packet_id(), 1);

        context.packet_id = 254;
        assert_eq!(context.next_packet_id(), 254);
        assert_eq!(context.next_packet_id(), 255);
        // First overflow: the declared restart, PACKET_ID_RESTART.
        assert_eq!(context.next_packet_id(), PACKET_ID_RESTART);
        assert_eq!(context.next_packet_id(), 1);
    }

    #[test]
    fn payload_size_is_the_packet_size_less_the_header() {
        let mut context = Context::new();
        assert_eq!(context.payload_size().unwrap(), 4096 - HEADER_BYTES);

        context.set_packet_size(9);
        assert_eq!(context.payload_size().unwrap(), 1);
    }

    #[test]
    fn payload_size_rejects_a_packet_that_cannot_carry_a_payload() {
        let mut context = Context::new();
        for size in [0, 1, 7, 8] {
            context.set_packet_size(size);
            assert_eq!(
                context.payload_size().unwrap_err(),
                Error::Protocol("negotiated packet size leaves no room for a payload".into()),
                "packet size {size}"
            );
        }
    }
}
