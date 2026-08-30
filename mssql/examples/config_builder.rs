//! Building a [`Config`] with the chainable [`Config::builder`] API instead
//! of `Config::new()` plus setters. Both styles produce the same `Config`
//! and remain fully supported — this example just shows the fluent form.
use mssql::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::builder()
        .host("localhost")
        .port(1433)
        .authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"))
        // On production, validate the server certificate instead — e.g. with
        // `.trust_cert_ca("path/to/ca.crt")` or the default OS trust store.
        .trust_cert()
        .build();

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&1i32]).await?;
    let row = stream.into_row().await?.unwrap();

    println!("{:?}", row);
    assert_eq!(Some(1), row.get(0));

    Ok(())
}
