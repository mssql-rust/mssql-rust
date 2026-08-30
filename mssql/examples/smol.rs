//! Connecting with the [smol](https://crates.io/crates/smol) runtime.
//! `smol::net::TcpStream` already implements the `futures-rs` traits `mssql`
//! needs, so — unlike Tokio — no compat wrapper is required.
use mssql::Config;
use once_cell::sync::Lazy;
use smol::net::TcpStream;
use std::env;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

#[cfg(not(all(windows, feature = "sql-browser-smol")))]
fn main() -> anyhow::Result<()> {
    smol::block_on(async {
        use mssql::Client;

        let config = Config::from_ado_string(&CONN_STR)?;

        let tcp = TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true)?;

        let mut client = Client::connect(config, tcp).await?;

        let stream = client.query("SELECT @P1", &[&1i32]).await?;
        let row = stream.into_row().await?.unwrap();

        println!("{:?}", row);
        assert_eq!(Some(1), row.get(0));

        Ok(())
    })
}

#[cfg(all(windows, feature = "sql-browser-smol"))]
fn main() -> anyhow::Result<()> {
    smol::block_on(async {
        use mssql::{Client, SqlBrowser};

        let config = Config::from_ado_string(&CONN_STR)?;

        let tcp = TcpStream::connect_named(&config).await?;
        tcp.set_nodelay(true)?;

        let mut client = Client::connect(config, tcp).await?;

        let stream = client.query("SELECT @P1", &[&1i32]).await?;
        let row = stream.into_row().await?.unwrap();

        println!("{:?}", row);
        assert_eq!(Some(1), row.get(0));

        Ok(())
    })
}
