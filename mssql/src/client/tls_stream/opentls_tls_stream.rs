use super::split_pem_certs;
use crate::{
    client::{
        config::{ClientCertSource, Config},
        TrustConfig,
    },
    error::{Error, IoErrorKind},
};
use futures_util::io::{AsyncRead, AsyncWrite};
pub(crate) use opentls::async_io::{TlsConnector, TlsStream};
use opentls::Certificate;
use std::fs;
use tracing::{event, Level};

/// The mutual-TLS identity for `Config::client_certificate_pkcs12`. Only
/// PKCS#12 is supported by this backend. Ported from upstream Tiberius
/// 4a212e7.
fn load_identity(source: &ClientCertSource) -> crate::Result<opentls::Identity> {
    match source {
        ClientCertSource::Pkcs12 { path, password } => {
            let buf = fs::read(path).map_err(|e| Error::Io {
                kind: IoErrorKind::InvalidData,
                message: format!("Could not read PKCS#12 identity {}: {e}", path.display()),
            })?;
            Ok(opentls::Identity::from_pkcs12(&buf, password)?)
        }
        ClientCertSource::CertAndKey { .. } => Err(Error::Tls(
            "the vendored-openssl backend only supports PKCS#12 client certificates; use \
             `Config::client_certificate_pkcs12`"
                .into(),
        )),
    }
}

pub(crate) async fn create_tls_stream<S: AsyncRead + AsyncWrite + Unpin + Send>(
    config: &Config,
    stream: S,
) -> crate::Result<TlsStream<S>> {
    let mut builder = TlsConnector::new();

    if config.encryption == crate::EncryptionLevel::Strict {
        // Not fatal: SQL Server infers TDS 8.0 purely from the TLS
        // handshake happening before any TDS bytes, independent of ALPN
        // (see `EncryptionLevel::Strict`'s doc comment).
        event!(
            Level::WARN,
            "vendored-openssl does not support requesting the TDS 8.0 ALPN protocol; proceeding without it. SQL Server will still recognize TDS 8.0 from the handshake ordering alone."
        );
    }

    match &config.trust {
        TrustConfig::CaCertificateLocation(path) => {
            if let Ok(buf) = fs::read(path) {
                let cert = match path.extension() {
                        Some(ext)
                        if ext.eq_ignore_ascii_case("pem")
                            || ext.eq_ignore_ascii_case("crt") =>
                            {
                                Some(Certificate::from_pem(&buf)?)
                            }
                        Some(ext) if ext.eq_ignore_ascii_case("der") => {
                            Some(Certificate::from_der(&buf)?)
                        }
                        Some(_) | None => return Err(Error::Io {
                            kind: IoErrorKind::InvalidInput,
                            message: "Provided CA certificate with unsupported file-extension! Supported types are pem, crt and der.".to_string()}),
                    };
                if let Some(c) = cert {
                    builder = builder.add_root_certificate(c);
                }
            } else {
                return Err(Error::Io {
                    kind: IoErrorKind::InvalidData,
                    message: "Could not read provided CA certificate!".to_string(),
                });
            }
        }
        TrustConfig::CaCertificateBundle(bundle) => {
            for cert in split_pem_certs(bundle) {
                builder = builder.add_root_certificate(Certificate::from_pem(cert)?);
            }
        }
        TrustConfig::TrustAll => {
            event!(
                Level::WARN,
                "Trusting the server certificate without validation."
            );

            builder = builder.danger_accept_invalid_certs(true);
            builder = builder.danger_accept_invalid_hostnames(true);
            builder = builder.use_sni(false);
        }
        TrustConfig::Default => {
            event!(Level::INFO, "Using default trust configuration.");
        }
    }

    let domain = config
        .host_name_in_certificate
        .as_deref()
        .unwrap_or_else(|| config.get_host());

    if let Some(source) = &config.client_cert {
        event!(
            Level::DEBUG,
            "Presenting a client certificate for mutual TLS."
        );
        builder = builder.identity(load_identity(source)?);
    }

    Ok(builder.connect(domain, stream).await?)
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docker/certs/mtls")
            .join(name)
    }

    #[test]
    fn pkcs12_identity_loads_and_a_wrong_password_fails() {
        let ok = ClientCertSource::Pkcs12 {
            path: fixture("client.p12"),
            password: zeroize::Zeroizing::new("test-password".into()),
        };
        assert!(load_identity(&ok).is_ok());

        let bad = ClientCertSource::Pkcs12 {
            path: fixture("client.p12"),
            password: zeroize::Zeroizing::new("wrong".into()),
        };
        assert!(load_identity(&bad).is_err());
    }
}
