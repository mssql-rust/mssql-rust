//! TLS behaviour that SQL Server itself can't be configured to exercise here,
//! verified against a local rustls server. TDS 8.0 Strict encryption does
//! the TLS handshake directly on the TCP stream, so a plain TLS server can
//! check:
//!
//! - that the Strict handshake is raw TLS asking for the `tds/8.0` ALPN
//!   protocol, not TLS records framed in TDS PRELOGIN packets;
//! - client-certificate (mutual TLS) authentication, `Config::client_certificate`
//!   (ported from upstream Tiberius 4a212e7).
//!
//! Fixtures are throwaway, test-only certificates in docker/certs/mtls/ (the
//! CA key was deleted after signing).

use mssql::{Config, EncryptionLevel};
use std::{sync::Arc, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};
use tokio_rustls::rustls::{
    self,
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
    RootCertStore, ServerConfig,
};
use tokio_util::compat::TokioAsyncWriteCompatExt;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/docker/certs/mtls");

#[derive(Debug)]
struct Seen {
    alpn: Option<Vec<u8>>,
    client_cert: Option<CertificateDer<'static>>,
    handshake: Result<(), String>,
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

async fn serve_once(require_client_cert: bool) -> (u16, oneshot::Receiver<Seen>) {
    let chain: Vec<_> = CertificateDer::pem_file_iter(format!("{DIR}/server.pem"))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(format!("{DIR}/server.key")).unwrap();

    let builder = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .unwrap();
    let builder = if require_client_cert {
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from_pem_file(format!("{DIR}/ca.pem")).unwrap())
            .unwrap();
        let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider())
            .build()
            .unwrap();
        builder.with_client_cert_verifier(verifier)
    } else {
        builder.with_no_client_auth()
    };
    let mut server = builder.with_single_cert(chain, key).unwrap();
    server.alpn_protocols = vec![b"tds/8.0".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let seen = match acceptor.accept(sock).await {
            Ok(mut tls) => {
                let (_, conn) = tls.get_ref();
                let seen = Seen {
                    alpn: conn.alpn_protocol().map(<[u8]>::to_vec),
                    client_cert: conn.peer_certificates().and_then(|c| c.first().cloned()),
                    handshake: Ok(()),
                };
                // The client's next bytes are its PRELOGIN, inside TLS.
                let mut byte = [0u8; 1];
                let _ = tokio::time::timeout(Duration::from_secs(2), tls.read(&mut byte)).await;
                seen
            }
            Err(e) => Seen {
                alpn: None,
                client_cert: None,
                handshake: Err(e.to_string()),
            },
        };
        let _ = tx.send(seen);
    });

    (port, rx)
}

async fn connect_strict(port: u16, client_cert: bool) -> mssql::Result<()> {
    let mut config = Config::new();
    config.host("localhost");
    config.port(port);
    config.encryption(EncryptionLevel::Strict);
    config.trust_cert_ca(format!("{DIR}/ca.pem"));
    config.handshake_timeout(Some(Duration::from_secs(5)));
    if client_cert {
        config.client_certificate(format!("{DIR}/client.pem"), format!("{DIR}/client.key"));
    }
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    // Our server isn't SQL Server, so this always fails after the
    // handshake; only what the server saw matters here.
    mssql::Client::connect(config, tcp.compat_write())
        .await
        .map(drop)
}

fn cn(cert: &CertificateDer<'_>) -> bool {
    cert.windows(b"mssql-test-client".len())
        .any(|w| w == b"mssql-test-client")
}

#[tokio::test]
async fn strict_handshake_is_raw_tls_with_tds8_alpn() {
    let (port, seen) = serve_once(false).await;
    let _ = connect_strict(port, false).await;
    let seen = seen.await.unwrap();
    assert_eq!(seen.handshake, Ok(()), "{seen:?}");
    assert_eq!(seen.alpn.as_deref(), Some(&b"tds/8.0"[..]));
    assert!(seen.client_cert.is_none());
}

#[tokio::test]
async fn client_certificate_is_presented_for_mutual_tls() {
    let (port, seen) = serve_once(true).await;
    let _ = connect_strict(port, true).await;
    let seen = seen.await.unwrap();
    assert_eq!(seen.handshake, Ok(()), "{seen:?}");
    assert!(seen.client_cert.as_ref().is_some_and(|c| cn(c)), "{seen:?}");
}

#[tokio::test]
async fn a_server_requiring_a_client_certificate_rejects_its_absence() {
    let (port, seen) = serve_once(true).await;
    assert!(connect_strict(port, false).await.is_err());
    let seen = seen.await.unwrap();
    assert!(seen.handshake.is_err(), "{seen:?}");
}

#[tokio::test]
async fn unreadable_client_certificate_files_are_errors() {
    let mut config = Config::new();
    config.host("localhost");
    config.port(1);
    config.encryption(EncryptionLevel::Strict);
    config.client_certificate("/nonexistent/client.pem", "/nonexistent/client.key");
    let (port, _seen) = serve_once(false).await;
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let err = mssql::Client::connect(config, tcp.compat_write())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("client certificate"), "{err}");
}
