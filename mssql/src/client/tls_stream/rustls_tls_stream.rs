use crate::{
    client::{
        config::{ClientCertSource, Config},
        TrustConfig,
    },
    error::IoErrorKind,
    Error,
};
use futures_util::io::{AsyncRead, AsyncWrite};
use std::{
    fs, io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio_rustls::{
    rustls::{
        client::{
            danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
            WantsClientCert,
        },
        crypto::{ring, CryptoProvider},
        pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName, UnixTime},
        ClientConfig, ConfigBuilder, DigitallySignedStruct, Error as RustlsError, RootCertStore,
        SignatureScheme, WantsVerifier,
    },
    TlsConnector,
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::{event, Level};

impl From<tokio_rustls::rustls::Error> for Error {
    fn from(e: tokio_rustls::rustls::Error) -> Self {
        crate::Error::Tls(e.to_string())
    }
}

/// The platform trust store, tolerating individual load/parse errors (they
/// are logged). May be empty, e.g. in a container without a CA bundle.
fn native_root_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    let mut valid_count = 0;
    let mut invalid_count = 0;

    let loaded = rustls_native_certs::load_native_certs();
    for err in &loaded.errors {
        event!(Level::WARN, "error loading a platform certificate: {}", err);
    }

    for cert in loaded.certs {
        match roots.add(cert) {
            Ok(_) => valid_count += 1,
            Err(err) => {
                event!(Level::DEBUG, "certificate parsing failed: {:?}", err);
                invalid_count += 1
            }
        }
    }
    event!(
        Level::TRACE,
        "native roots: {} valid and {} invalid certs",
        valid_count,
        invalid_count
    );
    roots
}

/// The CA certificate(s) in a `.pem`/`.crt` (every certificate in it; this
/// used to reject a file with more than one) or `.der` file.
fn read_ca_file(path: &std::path::Path) -> crate::Result<Vec<CertificateDer<'static>>> {
    let buf = fs::read(path).map_err(|e| Error::Io {
        kind: IoErrorKind::InvalidData,
        message: format!("Could not read CA certificate {}: {e}", path.display()),
    })?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let certs: Vec<CertificateDer<'static>> = match ext.as_str() {
        "pem" | "crt" => CertificateDer::pem_slice_iter(&buf)
            .collect::<Result<_, _>>()
            .map_err(|e| Error::Io {
                kind: IoErrorKind::InvalidData,
                message: format!("Failed to parse PEM certificate: {e}"),
            })?,
        "der" => vec![CertificateDer::from(buf)],
        _ => {
            return Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                message: "Provided CA certificate with unsupported file-extension! Supported types are pem, crt and der.".to_string(),
            })
        }
    };
    if certs.is_empty() {
        return Err(Error::Io {
            kind: IoErrorKind::InvalidInput,
            message: format!(
                "CA certificate file {} contains no certificates",
                path.display()
            ),
        });
    }
    Ok(certs)
}

/// `certs`, on top of the platform roots when `with_native_roots` (the
/// additive `trust_cert_ca`/`trust_cert_ca_bundle`), or alone (the pinning
/// `trust_cert_ca_only`).
fn ca_root_store(
    certs: Vec<CertificateDer<'static>>,
    with_native_roots: bool,
) -> crate::Result<RootCertStore> {
    let mut roots = if with_native_roots {
        native_root_store()
    } else {
        RootCertStore::empty()
    };
    for cert in certs {
        roots.add(cert)?;
    }
    Ok(roots)
}

/// Reads the client certificate chain and private key for mutual TLS
/// (`Config::client_certificate`). Ported from upstream Tiberius 4a212e7.
fn load_client_auth(
    source: &ClientCertSource,
) -> crate::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let (cert, key) = match source {
        ClientCertSource::CertAndKey { cert, key } => (cert, key),
        #[cfg(any(feature = "native-tls", feature = "vendored-openssl"))]
        ClientCertSource::Pkcs12 { .. } => {
            return Err(Error::Tls(
                "the rustls backend doesn't support PKCS#12 client certificates; pass \
                 separate certificate and key files to `Config::client_certificate`"
                    .into(),
            ))
        }
    };

    let read = |path: &std::path::Path, what: &str| {
        fs::read(path).map_err(|e| Error::Io {
            kind: IoErrorKind::InvalidData,
            message: format!("Could not read client {what} {}: {e}", path.display()),
        })
    };
    let ext = |path: &std::path::Path| {
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    let invalid = |message: String| Error::Io {
        kind: IoErrorKind::InvalidData,
        message,
    };

    let cert_buf = read(cert, "certificate")?;
    let chain: Vec<CertificateDer<'static>> = match ext(cert).as_str() {
        "pem" | "crt" => CertificateDer::pem_slice_iter(&cert_buf)
            .collect::<Result<_, _>>()
            .map_err(|e| invalid(format!("Failed to parse PEM client certificate: {e}")))?,
        "der" => vec![CertificateDer::from(cert_buf)],
        _ => {
            return Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                message: "Client certificate must be a .pem, .crt or .der file".into(),
            })
        }
    };
    if chain.is_empty() {
        return Err(invalid(format!(
            "Client certificate file {} contains no certificates",
            cert.display()
        )));
    }

    let key_buf = read(key, "private key")?;
    let key = match ext(key).as_str() {
        "pem" | "key" => PrivateKeyDer::from_pem_slice(&key_buf)
            .map_err(|e| invalid(format!("Failed to parse PEM private key: {e}")))?,
        "der" => PrivateKeyDer::try_from(key_buf)
            .map_err(|e| invalid(format!("Failed to parse DER private key: {e}")))?,
        _ => {
            return Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                message: "Client private key must be a .pem, .key or .der file".into(),
            })
        }
    };

    Ok((chain, key))
}

/// Turns a certificate-validation failure during the TLS handshake into an
/// `Error::Tls` that names the remedies (upstream issue #327). tokio-rustls
/// wraps the `rustls::Error` in an `io::Error`, and its raw message ("invalid
/// peer certificate ... UnsupportedCertVersion") never says what to do.
/// Every other I/O error passes through unchanged. Ported from upstream
/// Tiberius (tiberius-rs/tiberius 610154f).
fn map_handshake_error(err: io::Error, trust: &TrustConfig) -> Error {
    // Under TrustAll the verifier never rejects a certificate.
    if !matches!(trust, TrustConfig::TrustAll) {
        if let Some(RustlsError::InvalidCertificate(cert_err)) =
            err.get_ref().and_then(|e| e.downcast_ref::<RustlsError>())
        {
            let hint = if cert_err.to_string().contains("UnsupportedCertVersion") {
                " The server presented a certificate with an X.509 version rustls \
                 doesn't accept, which often happens with the self-signed certificate \
                 SQL Server generates for itself."
            } else {
                ""
            };

            return Error::Tls(format!(
                "the server's certificate was rejected during the TLS handshake: \
                 {cert_err}.{hint} To connect anyway: (1) trust its CA with \
                 `Config::trust_cert_ca(path)` or `trust_cert_ca_bundle(bytes)`; \
                 (2) skip validation with `Config::trust_cert()` (accepts any \
                 certificate; only safe on a trusted network); or (3) build mssql \
                 with the `native-tls` backend, which is more lenient with legacy \
                 certificates. SQL Server performs a TLS handshake during login even \
                 with `Encrypt=false`, so this can happen regardless of that setting."
            ));
        }
    }

    Error::from(err)
}

pub(crate) struct TlsStream<S: AsyncRead + AsyncWrite + Unpin + Send>(
    Compat<tokio_rustls::client::TlsStream<Compat<S>>>,
);

#[derive(Debug)]
struct NoCertVerifier;

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

fn get_server_name(config: &Config) -> crate::Result<ServerName<'static>> {
    let host = config
        .host_name_in_certificate
        .as_deref()
        .unwrap_or_else(|| config.get_host());

    match (ServerName::try_from(host), &config.trust) {
        (Ok(sn), _) => Ok(sn.to_owned()),
        (Err(_), TrustConfig::TrustAll) => {
            Ok(ServerName::try_from("placeholder.domain.com").unwrap())
        }
        (Err(e), _) => Err(crate::Error::Tls(e.to_string())),
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> TlsStream<S> {
    pub(super) async fn new(config: &Config, stream: S) -> crate::Result<Self> {
        event!(Level::INFO, "Performing a TLS handshake");

        // Honor a process-wide CryptoProvider if the application installed one
        // (`CryptoProvider::install_default`), e.g. aws-lc-rs or a FIPS
        // provider; otherwise use this crate's default, ring. Ported from
        // upstream Tiberius 7235f4c, with the fallback kept at ring.
        let provider = CryptoProvider::get_default()
            .cloned()
            .unwrap_or_else(|| Arc::new(ring::default_provider()));

        // TDS 8.0 Strict (TLS directly on TCP) may negotiate TLS 1.2 or 1.3.
        // The older TDS 7.x handshake (TLS records wrapped in PRELOGIN
        // packets, for every other encryption level) supports TLS only up to
        // 1.2, per Microsoft's TDS 8.0 compatibility table. Verified live: a
        // SQL Server 2022 allowing TLS 1.3 negotiates it on the TDS 7.x path
        // and the connection then breaks with "received corrupt message of
        // type InvalidContentType". So offer 1.3 only for Strict.
        let versions: &[&'static tokio_rustls::rustls::SupportedProtocolVersion] =
            if config.encryption == crate::EncryptionLevel::Strict {
                tokio_rustls::rustls::DEFAULT_VERSIONS
            } else {
                &[&tokio_rustls::rustls::version::TLS12]
            };
        let builder = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(versions)
            .map_err(|e| crate::Error::Tls(e.to_string()))?;

        let wants_client_cert: ConfigBuilder<ClientConfig, WantsClientCert> = match &config.trust {
            // trust_cert_ca / trust_cert_ca_bundle add to the system roots,
            // as on native-tls and vendored-openssl (and as documented).
            // They used to *replace* them here. trust_cert_ca_only keeps
            // that pinning behaviour, explicitly.
            TrustConfig::CaCertificateLocation(path) => {
                builder.with_root_certificates(ca_root_store(read_ca_file(path)?, true)?)
            }
            TrustConfig::CaCertificateOnly(path) => {
                builder.with_root_certificates(ca_root_store(read_ca_file(path)?, false)?)
            }
            TrustConfig::CaCertificateBundle(bundle) => {
                let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(bundle)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| crate::Error::Io {
                        kind: IoErrorKind::InvalidData,
                        message: format!("Failed to parse PEM certificate bundle: {e}"),
                    })?;
                if certs.is_empty() {
                    return Err(crate::Error::Io {
                        kind: IoErrorKind::InvalidInput,
                        message: "Provided CA certificate bundle contains no certificates"
                            .to_string(),
                    });
                }
                builder.with_root_certificates(ca_root_store(certs, true)?)
            }
            TrustConfig::TrustAll => {
                event!(
                    Level::WARN,
                    "Trusting the server certificate without validation."
                );
                builder
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
            }
            TrustConfig::Default => {
                event!(Level::INFO, "Using default trust configuration.");
                builder.with_native_roots()?
            }
            #[cfg(feature = "rustls-webpki-roots")]
            TrustConfig::WebPkiRoots => {
                event!(Level::INFO, "Using webpki-roots trust configuration.");
                builder.with_webpki_roots()
            }
        };

        let mut client_config = match &config.client_cert {
            Some(source) => {
                event!(
                    Level::DEBUG,
                    "Presenting a client certificate for mutual TLS."
                );
                let (chain, key) = load_client_auth(source)?;
                wants_client_cert
                    .with_client_auth_cert(chain, key)
                    .map_err(|e| crate::Error::Tls(e.to_string()))?
            }
            None => wants_client_cert.with_no_client_auth(),
        };

        if config.encryption == crate::EncryptionLevel::Strict {
            client_config
                .alpn_protocols
                .push(super::TDS_ALPN_PROTOCOL_NAME.as_bytes().to_vec());
        }

        let connector = TlsConnector::from(Arc::new(client_config));

        let tls_stream = connector
            .connect(get_server_name(config)?, stream.compat())
            .await
            .map_err(|e| map_handshake_error(e, &config.trust))?;

        Ok(TlsStream(tls_stream.compat()))
    }

    pub(crate) fn get_mut(&mut self) -> &mut S {
        self.0.get_mut().get_mut().0.get_mut()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncRead for TlsStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_read(cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncWrite for TlsStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_close(cx)
    }
}

trait ConfigBuilderExt {
    fn with_native_roots(self) -> crate::Result<ConfigBuilder<ClientConfig, WantsClientCert>>;

    #[cfg(feature = "rustls-webpki-roots")]
    fn with_webpki_roots(self) -> ConfigBuilder<ClientConfig, WantsClientCert>;
}

impl ConfigBuilderExt for ConfigBuilder<ClientConfig, WantsVerifier> {
    // Used to `.expect()` the platform cert load, which in rustls-native-certs
    // 0.8 panics if *any* error occurred (e.g. one unreadable file under
    // SSL_CERT_DIR), and to `assert!` a non-empty store, which panicked on
    // every connect in a container without CA certificates. Partial load
    // errors are now logged and tolerated, and an empty store is a TLS error
    // (fails closed). Ported from upstream Tiberius afdf8b3 / 5885e11 / 3e6afc3.
    fn with_native_roots(self) -> crate::Result<ConfigBuilder<ClientConfig, WantsClientCert>> {
        let roots = native_root_store();
        if roots.is_empty() {
            return Err(crate::Error::Tls(
                "no CA certificates found in the platform trust store; install the system \
                 CA bundle, or use trust_cert_ca / trust_cert_ca_bundle / trust_webpki_roots"
                    .into(),
            ));
        }

        Ok(self.with_root_certificates(roots))
    }

    #[cfg(feature = "rustls-webpki-roots")]
    fn with_webpki_roots(self) -> ConfigBuilder<ClientConfig, WantsClientCert> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        self.with_root_certificates(roots)
    }
}

#[cfg(test)]
mod handshake_error_tests {
    use super::*;
    use tokio_rustls::rustls::{CertificateError, OtherError};

    fn cert_io_error(cert_err: CertificateError) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            RustlsError::InvalidCertificate(cert_err),
        )
    }

    fn temporary_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("mssql-rustls-{}-{name}", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    // webpki's UnsupportedCertVersion reaches rustls as
    // `CertificateError::Other(..)` whose Debug text is that name.
    #[derive(Debug)]
    struct UnsupportedCertVersion;
    impl std::fmt::Display for UnsupportedCertVersion {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("UnsupportedCertVersion")
        }
    }
    impl std::error::Error for UnsupportedCertVersion {}

    #[test]
    fn certificate_failures_become_actionable_tls_errors() {
        let other = CertificateError::Other(OtherError(Arc::new(UnsupportedCertVersion)));
        let msg = map_handshake_error(cert_io_error(other), &TrustConfig::Default).to_string();
        for needle in [
            "trust_cert_ca",
            "trust_cert()",
            "native-tls",
            "X.509 version",
        ] {
            assert!(msg.contains(needle), "{needle}: {msg}");
        }

        let err = map_handshake_error(
            cert_io_error(CertificateError::UnknownIssuer),
            &TrustConfig::Default,
        );
        assert!(matches!(err, Error::Tls(_)), "{err:?}");
        assert!(!err.to_string().contains("X.509 version"));
    }

    #[test]
    fn other_io_errors_pass_through() {
        let err = map_handshake_error(
            io::Error::new(io::ErrorKind::ConnectionReset, "reset"),
            &TrustConfig::Default,
        );
        assert!(matches!(err, Error::Io { .. }), "{err:?}");

        let err = map_handshake_error(
            cert_io_error(CertificateError::UnknownIssuer),
            &TrustConfig::TrustAll,
        );
        assert!(matches!(err, Error::Io { .. }), "{err:?}");
    }

    #[test]
    fn process_default_provider_is_honored_or_falls_back_to_ring() {
        // Whatever is installed (nothing, in this test binary), building a
        // config with the resolved provider must work.
        let provider = CryptoProvider::get_default()
            .cloned()
            .unwrap_or_else(|| Arc::new(ring::default_provider()));
        assert!(ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .is_ok());
    }

    // trust_cert_ca adds the CA to the platform roots; trust_cert_ca_only
    // trusts it alone.
    #[test]
    fn ca_trust_is_additive_unless_pinned() {
        let ca = read_ca_file(&std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docker/certs/mtls/ca.pem"
        )))
        .unwrap();
        let native = native_root_store().len();

        let additive = ca_root_store(ca.clone(), true).unwrap();
        assert_eq!(additive.len(), native + 1);
        let pinned = ca_root_store(ca, false).unwrap();
        assert_eq!(pinned.len(), 1);
        // A hermetic container or an instrumented test process may have no
        // discoverable platform roots. The size delta above still proves the
        // configured CA is added rather than replacing whatever was found.
    }

    #[test]
    fn certificate_file_errors_are_typed_and_actionable() {
        let missing = std::path::Path::new("/definitely/missing/mssql-ca.pem");
        assert!(matches!(read_ca_file(missing), Err(Error::Io { .. })));

        let unsupported = temporary_file("ca.txt", b"certificate");
        assert!(matches!(
            read_ca_file(&unsupported),
            Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                ..
            })
        ));

        let empty = temporary_file("empty.pem", b"");
        assert!(matches!(
            read_ca_file(&empty),
            Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                ..
            })
        ));

        let malformed = temporary_file("malformed.pem", b"-----BEGIN CERTIFICATE-----\n!");
        assert!(matches!(
            read_ca_file(&malformed),
            Err(Error::Io {
                kind: IoErrorKind::InvalidData,
                ..
            })
        ));

        let der = temporary_file("opaque.der", &[1, 2, 3]);
        assert_eq!(read_ca_file(&der).unwrap().len(), 1);
        assert!(ca_root_store(read_ca_file(&der).unwrap(), false).is_err());
    }

    #[test]
    fn client_auth_rejects_missing_empty_and_wrongly_named_files() {
        let source = |cert, key| ClientCertSource::CertAndKey { cert, key };
        let missing = std::path::PathBuf::from("/definitely/missing/client.pem");
        assert!(matches!(
            load_client_auth(&source(missing, "key.pem".into())),
            Err(Error::Io { .. })
        ));

        let wrong_cert = temporary_file("client.txt", b"certificate");
        assert!(matches!(
            load_client_auth(&source(wrong_cert, "key.pem".into())),
            Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                ..
            })
        ));

        let empty_cert = temporary_file("client-empty.pem", b"");
        assert!(matches!(
            load_client_auth(&source(empty_cert, "key.pem".into())),
            Err(Error::Io {
                kind: IoErrorKind::InvalidData,
                ..
            })
        ));

        let malformed_cert = temporary_file(
            "client-malformed.pem",
            b"-----BEGIN CERTIFICATE-----\n!\n-----END CERTIFICATE-----\n",
        );
        assert!(matches!(
            load_client_auth(&source(malformed_cert, "key.pem".into())),
            Err(Error::Io {
                kind: IoErrorKind::InvalidData,
                ..
            })
        ));

        let cert = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docker/certs/mtls/client.pem"
        ));
        let wrong_key = temporary_file("client-key.txt", b"key");
        assert!(matches!(
            load_client_auth(&source(cert.clone(), wrong_key)),
            Err(Error::Io {
                kind: IoErrorKind::InvalidInput,
                ..
            })
        ));
        assert!(matches!(
            load_client_auth(&source(
                cert.clone(),
                "/definitely/missing/client.key".into()
            )),
            Err(Error::Io { .. })
        ));

        let malformed_key = temporary_file("client-bad.key", b"not a key");
        assert!(matches!(
            load_client_auth(&source(cert.clone(), malformed_key)),
            Err(Error::Io {
                kind: IoErrorKind::InvalidData,
                ..
            })
        ));
        let malformed_der = temporary_file("client-bad.der", b"not a key");
        assert!(matches!(
            load_client_auth(&source(cert, malformed_der)),
            Err(Error::Io {
                kind: IoErrorKind::InvalidData,
                ..
            })
        ));
    }

    #[test]
    fn server_name_validation_only_allows_placeholders_when_trust_is_disabled() {
        let mut config = Config::new();
        config.host("not a valid DNS name");
        assert!(matches!(get_server_name(&config), Err(Error::Tls(_))));

        config.trust_cert();
        assert_eq!(
            get_server_name(&config).unwrap().to_str(),
            "placeholder.domain.com"
        );

        config.host_name_in_certificate("sql.example.com");
        assert_eq!(
            get_server_name(&config).unwrap().to_str(),
            "sql.example.com"
        );
    }

    #[test]
    fn rustls_errors_convert_to_public_tls_errors() {
        let error: Error = RustlsError::General("handshake failed".into()).into();
        assert!(matches!(error, Error::Tls(message) if message.contains("handshake failed")));
    }

    #[tokio::test]
    async fn certificate_bundle_errors_are_rejected_before_network_io() {
        for (bundle, expected_kind) in [
            (&b""[..], IoErrorKind::InvalidInput),
            (
                &b"-----BEGIN CERTIFICATE-----\n!\n-----END CERTIFICATE-----\n"[..],
                IoErrorKind::InvalidData,
            ),
        ] {
            let mut config = Config::new();
            config.trust_cert_ca_bundle(bundle);
            let stream = futures_util::io::Cursor::new(Vec::<u8>::new());
            let error = match TlsStream::new(&config, stream).await {
                Ok(_) => panic!("invalid CA bundle unexpectedly reached a TLS connection"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                Error::Io { kind, .. } if kind == expected_kind
            ));
        }
    }
}
