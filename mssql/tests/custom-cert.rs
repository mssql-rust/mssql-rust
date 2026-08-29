#![cfg(unix)]
use mssql::{AuthMethod, Client, Config, EncryptionLevel, Result};
use std::sync::Once;
use tokio::{net::TcpStream, runtime::Runtime};
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[allow(dead_code)]
static LOGGER_SETUP: Once = Once::new();

#[test]
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
fn connect_to_custom_cert_instance_ado() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        let mut config = Config::from_ado_string("server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificateCA=docker/certs/customCA.crt")?;
        config.authentication(AuthMethod::sql_server(
            "sa",
            "<YourStrong@Passw0rd>",
        ));

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let mut client = Client::connect(config, tcp.compat_write()).await?;

        let row = client
            .query("SELECT @P1", &[&-4i32])
            .await?
            .into_row()
            .await?
            .unwrap();

        assert_eq!(Some(-4i32), row.get(0));
        Ok(())
    })
}

#[test]
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
fn connect_to_custom_cert_instance_jdbc() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        // Careful: the / in the TrustServerCertificateCA needs to be escaped
        let mut config = Config::from_jdbc_string(
            "jdbc:sqlserver://localhost:1433;TrustServerCertificateCA=docker{/}certs{/}customCA.crt",
        )?;
        config.authentication(AuthMethod::sql_server("sa", "<YourStrong@Passw0rd>"));

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let mut client = Client::connect(config, tcp.compat_write()).await?;

        let row = client
            .query("SELECT @P1", &[&-4i32])
            .await?
            .into_row()
            .await?
            .unwrap();

        assert_eq!(Some(-4i32), row.get(0));
        Ok(())
    })
}

// Regression test for prisma/tiberius#290: `Config::trust_cert_ca_bundle`
// takes the CA certificate's PEM bytes directly (rather than a filesystem
// path, as `trust_cert_ca` does) - here read from the same file the other
// tests in this module reference by path, to prove both entry points trust
// an equivalent certificate.
#[test]
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
fn connect_to_custom_cert_instance_via_bundle_bytes() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        let ca_bytes = std::fs::read("docker/certs/customCA.crt")?;

        let mut config = Config::new();
        config.host("localhost");
        config.port(1433);
        config.trust_cert_ca_bundle(ca_bytes);
        config.authentication(AuthMethod::sql_server("sa", "<YourStrong@Passw0rd>"));

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let mut client = Client::connect(config, tcp.compat_write()).await?;

        let row = client
            .query("SELECT @P1", &[&-4i32])
            .await?
            .into_row()
            .await?
            .unwrap();

        assert_eq!(Some(-4i32), row.get(0));
        Ok(())
    })
}

// A CA bundle can hold more than one certificate concatenated together; a
// second, unrelated (self-signed, unused) certificate placed before the
// real one in the bundle must not prevent the real one from being found
// and trusted.
#[test]
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
fn connect_to_custom_cert_instance_via_multi_cert_bundle() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        // A genuine, unrelated, throwaway self-signed certificate (not the
        // test server's own CA) - proves a multi-certificate bundle works
        // by having something else, still valid, ahead of the real one.
        let decoy = b"-----BEGIN CERTIFICATE-----\n\
            MIIBhTCCASugAwIBAgIUKUgjm1O5PsI3OlADc1JNa5Lr2/IwCgYIKoZIzj0EAwIw\n\
            GDEWMBQGA1UEAwwNRGVjb3lVbnVzZWRDQTAeFw0yNjA4MjkxNzQ5MTBaFw0zNjA4\n\
            MjYxNzQ5MTBaMBgxFjAUBgNVBAMMDURlY295VW51c2VkQ0EwWTATBgcqhkjOPQIB\n\
            BggqhkjOPQMBBwNCAAQxWJ6GWvj7baJJVC/x7RrN3dnve5jRLMaWkM7jY9HBfmQ9\n\
            YHG/znLTjprzFqV1JeVQoqwKl0FT826STd0CulFxo1MwUTAdBgNVHQ4EFgQUJgDe\n\
            mYKFOgVcO4MTWzAzmbZ6FuAwHwYDVR0jBBgwFoAUJgDemYKFOgVcO4MTWzAzmbZ6\n\
            FuAwDwYDVR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAgNIADBFAiEAqI9aV3WXo7lM\n\
            TSx63kYQzbpqeuhOWE4CidD7cyF1d64CIFNxKr5FWWHbww56xHpCSK92Q568JAoI\n\
            QE3RMQwkoNkp\n\
            -----END CERTIFICATE-----\n";
        let real = std::fs::read("docker/certs/customCA.crt")?;
        let bundle = [decoy.as_slice(), &real].concat();

        let mut config = Config::new();
        config.host("localhost");
        config.port(1433);
        config.trust_cert_ca_bundle(bundle);
        config.authentication(AuthMethod::sql_server("sa", "<YourStrong@Passw0rd>"));

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let mut client = Client::connect(config, tcp.compat_write()).await?;

        let row = client
            .query("SELECT @P1", &[&-4i32])
            .await?
            .into_row()
            .await?
            .unwrap();

        assert_eq!(Some(-4i32), row.get(0));
        Ok(())
    })
}

#[test]
fn connect_to_custom_cert_instance_without_ca() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        let mut config = Config::new();
        config.authentication(AuthMethod::sql_server("sa", "<YourStrong@Passw0rd>"));
        config.encryption(EncryptionLevel::On);
        config.host("localhost");
        config.port(1433);

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let client = Client::connect(config, tcp.compat_write()).await;

        assert!(client.is_err());
        Ok(())
    })
}

// Regression test for prisma/tiberius#330 (webpki-roots support). This test
// server's certificate is self-signed for local testing, not issued by any
// public CA - so validating it against Mozilla's public root CA list
// (`trust_webpki_roots`) must fail, the same way it would for any real
// self-signed certificate. This can't positively prove webpki-roots
// validates a *real* publicly-trusted certificate (that would need an
// actual internet-facing SQL Server), but it does prove the roots are
// genuinely loaded and enforced rather than a no-op stub that would trust
// anything.
#[test]
#[cfg(feature = "rustls-webpki-roots")]
fn connect_to_custom_cert_instance_via_webpki_roots_rejects_self_signed_cert() -> Result<()> {
    LOGGER_SETUP.call_once(|| {
        env_logger::init();
    });

    let rt = Runtime::new()?;

    rt.block_on(async {
        let mut config = Config::new();
        config.host("localhost");
        config.port(1433);
        config.trust_webpki_roots();
        config.authentication(AuthMethod::sql_server("sa", "<YourStrong@Passw0rd>"));

        let tcp = TcpStream::connect(config.get_addr()).await?;

        let client = Client::connect(config, tcp.compat_write()).await;

        assert!(client.is_err());
        Ok(())
    })
}
