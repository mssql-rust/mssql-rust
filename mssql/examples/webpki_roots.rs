//! Validating the server's TLS certificate against Mozilla's bundled root CA
//! list ([`Config::trust_webpki_roots`]) instead of the operating system's
//! certificate store — useful in minimal/scratch containers that don't ship
//! one. Requires the `rustls-webpki-roots` feature (which implies `rustls`):
//!
//! ```sh
//! cargo run --example webpki_roots --no-default-features --features=tds73,rustls-webpki-roots
//! ```
//!
//! This only validates certificates issued by a public CA — it will reject
//! a self-signed development certificate, which is why this example doesn't
//! run against this crate's own local test server.
use mssql::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();

    config.host("your-server.database.windows.net");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
    config.trust_webpki_roots();

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&1i32]).await?;
    let row = stream.into_row().await?.unwrap();

    println!("{:?}", row);
    assert_eq!(Some(1), row.get(0));

    Ok(())
}
