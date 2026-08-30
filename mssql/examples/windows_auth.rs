//! Connecting with a Windows/NTLM username and password
//! ([`AuthMethod::windows`]).
//!
//! This works on Windows out of the box (the default `winauth` feature), or
//! on Unix with the pure-Rust `sspi-rs` feature enabled — neither needs a
//! real Kerberos ticket cache, unlike `AuthMethod::Integrated` with the
//! `integrated-auth-gssapi` feature. Run on Unix with:
//!
//! ```sh
//! cargo run --example windows_auth --no-default-features --features=tds73,rustls,sspi-rs
//! ```
#[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs")))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use mssql::{AuthMethod, Client, Config};
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncWriteCompatExt;

    let mut config = Config::new();

    config.host("localhost");
    config.port(1433);
    // A `DOMAIN\user` form splits into the domain and user automatically.
    config.authentication(AuthMethod::windows(r"DOMAIN\user", "password"));
    config.trust_cert();

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&1i32]).await?;
    let row = stream.into_row().await?.unwrap();

    println!("{:?}", row);
    assert_eq!(Some(1), row.get(0));

    Ok(())
}

#[cfg(not(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"))))]
fn main() {
    eprintln!(
        "This example needs `AuthMethod::windows`, available on Windows with the default \
         `winauth` feature, or on Unix with `--features=sspi-rs`."
    );
}
