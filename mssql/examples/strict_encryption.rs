//! Connecting with TDS 8.0 "Strict" encryption (`Encrypt=Strict`).
//!
//! Unlike the other [`EncryptionLevel`]s, the TLS handshake happens *before*
//! any TDS bytes at all, closing a downgrade window the other levels leave
//! open. This needs SQL Server 2022 or later, or Azure SQL Database/Managed
//! Instance — connecting with `Strict` to an older server fails the TLS
//! handshake outright, since the server doesn't expect a TLS ClientHello as
//! the very first bytes on the wire.
//!
//! Because `TrustServerCertificate=true` defeats the protection Strict mode
//! is for, pair it with [`Config::host_name_in_certificate`] to validate the
//! certificate's name instead, matching the driver-agnostic guidance in
//! Microsoft's own TDS 8.0 documentation.
use mssql::{AuthMethod, Client, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();

    config.host("localhost");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
    config.encryption(EncryptionLevel::Strict);

    // The name on the server's certificate, if different from `host` above
    // (e.g. connecting through a load balancer or a proxy).
    config.host_name_in_certificate("sql.example.com");
    config.trust_cert_ca("path/to/ca.crt");

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&1i32]).await?;
    let row = stream.into_row().await?.unwrap();

    println!("{:?}", row);
    assert_eq!(Some(1), row.get(0));

    Ok(())
}
