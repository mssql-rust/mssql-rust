//! Trusting a CA certificate bundle held in memory, rather than a file on
//! disk, via [`Config::trust_cert_ca_bundle`] — useful when the certificate
//! comes from a secret manager, an embedded asset, or an environment
//! variable. The bundle may hold more than one PEM-encoded certificate
//! concatenated together (a standard `ca-bundle.crt`-style file).
use mssql::{AuthMethod, Client, Config};
use std::env;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ca_bundle = env::var("MSSQL_CA_BUNDLE").expect(
        "Set MSSQL_CA_BUNDLE to the PEM-encoded CA certificate (or bundle of certificates) \
         that issued your SQL Server's TLS certificate.",
    );

    let mut config = Config::new();

    config.host("localhost");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
    config.trust_cert_ca_bundle(ca_bundle.into_bytes());

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&1i32]).await?;
    let row = stream.into_row().await?.unwrap();

    println!("{:?}", row);
    assert_eq!(Some(1), row.get(0));

    Ok(())
}
