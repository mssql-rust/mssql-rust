use crate::Config;
use futures_util::io::{AsyncRead, AsyncWrite};

/// The ALPN protocol identifier SQL Server uses to recognize a TDS 8.0
/// ("strict" encryption) connection during the TLS handshake. Requesting
/// it is optional - SQL Server infers TDS 8.0 purely from receiving a TLS
/// ClientHello before any TDS bytes at all (see
/// [`crate::EncryptionLevel::Strict`]) - but rustls, this crate's only
/// backend that actually requests it, does so anyway as documented
/// practice (native-tls and vendored-openssl just log that they can't).
///
/// Gated on `feature = "rustls"` alone rather than "not compiled dead code
/// under some other combo": since rustls always wins the priority order
/// established for #308 (rustls > vendored-openssl > native-tls) whenever
/// it's enabled, the bare feature flag already means it's the active
/// backend - unlike native-tls/vendored-openssl, which need an explicit
/// `not(feature = "rustls")` guard for their own shared helpers.
#[cfg(feature = "rustls")]
pub(crate) const TDS_ALPN_PROTOCOL_NAME: &str = "tds/8.0";

/// Splits a byte buffer holding one or more concatenated PEM-encoded X.509
/// certificates (a "CA bundle", e.g. curl's `ca-bundle.crt`) into the
/// individual `-----BEGIN CERTIFICATE----- ... -----END CERTIFICATE-----`
/// blocks, each still including its own BEGIN/END markers - the form
/// `native-tls`'s and `vendored-openssl`'s `Certificate::from_pem` each
/// expect (unlike rustls, whose `pki_types::pem::PemObject::pem_slice_iter`
/// natively iterates a multi-certificate PEM buffer, so it needs no
/// equivalent helper).
///
/// Only matches the `CERTIFICATE` PEM label specifically, not any
/// `-----BEGIN` marker, so a bundle file that also happens to contain
/// other PEM block types (private keys, CRLs, etc.) doesn't get
/// misinterpreted as certificate data.
// Matches exactly the condition under which `native_tls_stream`/
// `opentls_tls_stream` (this function's only callers) are the active
// backend module below - i.e. rustls is not enabled, since rustls takes
// priority whenever more than one TLS feature is enabled at once (see the
// `cfg_if!` below). Gating on the bare feature flags instead (`native-tls`
// or `vendored-openssl`) would make this dead code whenever rustls is
// additionally enabled, since Cargo's additive `--features` means
// `--features=rustls` alone still leaves the default `native-tls` feature
// on too.
#[cfg(all(
    not(feature = "rustls"),
    any(feature = "native-tls", feature = "vendored-openssl")
))]
pub(crate) fn split_pem_certs(bundle: &[u8]) -> impl Iterator<Item = &[u8]> {
    const BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    const END: &[u8] = b"-----END CERTIFICATE-----";

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    let mut rest = bundle;

    std::iter::from_fn(move || {
        let start = find(rest, BEGIN)?;
        let after_begin = &rest[start + BEGIN.len()..];

        let Some(end_offset) = find(after_begin, END) else {
            // A BEGIN with no matching END: not a well-formed bundle: stop
            // rather than yield a truncated/garbage block.
            rest = &[];
            return None;
        };

        let block_end = start + BEGIN.len() + end_offset + END.len();
        let block = &rest[start..block_end];
        rest = &rest[block_end..];

        Some(block)
    })
}

#[cfg(all(
    test,
    not(feature = "rustls"),
    any(feature = "native-tls", feature = "vendored-openssl")
))]
mod split_pem_certs_tests {
    use super::split_pem_certs;

    const ONE: &str = "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n";
    const TWO: &str = "-----BEGIN CERTIFICATE-----\nBBBB\n-----END CERTIFICATE-----\n";

    #[test]
    fn empty_bundle_yields_nothing() {
        assert_eq!(0, split_pem_certs(b"").count());
    }

    #[test]
    fn non_pem_bundle_yields_nothing() {
        assert_eq!(0, split_pem_certs(b"not a certificate at all").count());
    }

    #[test]
    fn single_cert_bundle_yields_one_block() {
        let certs: Vec<_> = split_pem_certs(ONE.as_bytes()).collect();
        assert_eq!(1, certs.len());
        assert_eq!(ONE.trim_end().as_bytes(), certs[0]);
    }

    #[test]
    fn two_cert_bundle_yields_two_blocks_in_order() {
        let bundle = format!("{ONE}{TWO}");
        let certs: Vec<_> = split_pem_certs(bundle.as_bytes()).collect();

        assert_eq!(2, certs.len());
        assert_eq!(ONE.trim_end().as_bytes(), certs[0]);
        assert_eq!(TWO.trim_end().as_bytes(), certs[1]);
    }

    #[test]
    fn ignores_leading_and_trailing_junk_between_blocks() {
        let bundle = format!("# comment\n{ONE}\n\n{TWO}# trailing comment\n");
        let certs: Vec<_> = split_pem_certs(bundle.as_bytes()).collect();

        assert_eq!(2, certs.len());
        assert_eq!(ONE.trim_end().as_bytes(), certs[0]);
        assert_eq!(TWO.trim_end().as_bytes(), certs[1]);
    }

    #[test]
    fn ignores_non_certificate_pem_blocks() {
        let key = "-----BEGIN PRIVATE KEY-----\nCCCC\n-----END PRIVATE KEY-----\n";
        let bundle = format!("{key}{ONE}");
        let certs: Vec<_> = split_pem_certs(bundle.as_bytes()).collect();

        assert_eq!(1, certs.len());
        assert_eq!(ONE.trim_end().as_bytes(), certs[0]);
    }

    #[test]
    fn unterminated_block_is_dropped_rather_than_yielded_truncated() {
        let bundle = format!("{ONE}-----BEGIN CERTIFICATE-----\nno end marker");
        let certs: Vec<_> = split_pem_certs(bundle.as_bytes()).collect();

        assert_eq!(1, certs.len());
        assert_eq!(ONE.trim_end().as_bytes(), certs[0]);
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "rustls")] {
        mod rustls_tls_stream;

        pub(crate) use rustls_tls_stream::TlsStream;

        pub(crate) async fn create_tls_stream<S: AsyncRead + AsyncWrite + Unpin + Send>(
            config: &Config,
            stream: S,
        ) -> crate::Result<TlsStream<S>> {
            TlsStream::new(config, stream).await
        }
    } else if #[cfg(feature = "vendored-openssl")] {
        mod opentls_tls_stream;

        pub(crate) use opentls_tls_stream::TlsStream;

        pub(crate) async fn create_tls_stream<S: AsyncRead + AsyncWrite + Unpin + Send>(
            config: &Config,
            stream: S,
        ) -> crate::Result<TlsStream<S>> {
            opentls_tls_stream::create_tls_stream(config, stream).await
        }
    } else if #[cfg(feature = "native-tls")] {
        mod native_tls_stream;

        pub(crate) use native_tls_stream::TlsStream;

        pub(crate) async fn create_tls_stream<S: AsyncRead + AsyncWrite + Unpin + Send>(
            config: &Config,
            stream: S,
        ) -> crate::Result<TlsStream<S>> {
            native_tls_stream::create_tls_stream(config, stream).await
        }
    }
}
