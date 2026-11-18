//! `Config::handshake_timeout` / `Config::command_timeout`, ported from
//! upstream Tiberius (tiberius-rs/tiberius 2c63330; upstream issues #375,
//! #360).

use mssql::Config;
use once_cell::sync::Lazy;
use std::{env, time::Duration};
use tokio_util::compat::TokioAsyncWriteCompatExt;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

type Client = mssql::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

async fn connect(command_timeout: Option<Duration>) -> mssql::Result<Client> {
    let mut config = Config::from_ado_string(&CONN_STR)?;
    config.command_timeout(command_timeout);
    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;
    mssql::Client::connect(config, tcp.compat_write()).await
}

fn is_timed_out(err: &mssql::error::Error) -> bool {
    matches!(
        err,
        mssql::error::Error::Io {
            kind: std::io::ErrorKind::TimedOut,
            ..
        }
    )
}

#[tokio::test]
async fn a_stalled_command_times_out_and_desyncs_the_connection() -> mssql::Result<()> {
    let mut conn = connect(Some(Duration::from_secs(1))).await?;

    let started = std::time::Instant::now();
    let err = async {
        conn.simple_query("WAITFOR DELAY '00:00:03'; SELECT 1")
            .await?
            .into_results()
            .await
    }
    .await
    .unwrap_err();
    assert!(is_timed_out(&err), "{err:?}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );

    match conn.simple_query("SELECT 1").await {
        Err(mssql::error::Error::Protocol(msg)) => {
            assert!(msg.contains("command timeout"), "{msg}")
        }
        other => panic!("expected the desync error, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn the_default_bound_lets_a_short_wait_through() -> mssql::Result<()> {
    let mut conn = {
        let config = Config::from_ado_string(&CONN_STR)?;
        assert_eq!(config.get_command_timeout(), Some(Duration::from_secs(30)));
        let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
        mssql::Client::connect(config, tcp.compat_write()).await?
    };
    let row = conn
        .simple_query("WAITFOR DELAY '00:00:01'; SELECT 42")
        .await?
        .into_row()
        .await?
        .unwrap();
    assert_eq!(Some(42i32), row.get(0));
    Ok(())
}

#[tokio::test]
async fn no_bound_waits_indefinitely() -> mssql::Result<()> {
    let mut conn = connect(None).await?;
    let row = conn
        .simple_query("WAITFOR DELAY '00:00:02'; SELECT 1")
        .await?
        .into_row()
        .await?
        .unwrap();
    assert_eq!(Some(1i32), row.get(0));
    Ok(())
}

// A peer that accepts TCP but never answers PRELOGIN: the handshake bound
// must fire instead of hanging. No SQL Server needed.
#[tokio::test]
async fn a_server_that_stalls_the_handshake_times_out() -> mssql::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let _hold = tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(sock);
    });

    let mut config = Config::new();
    config.host("127.0.0.1");
    config.port(addr.port());
    config.handshake_timeout(Some(Duration::from_millis(300)));
    let tcp = tokio::net::TcpStream::connect(addr).await?;

    let started = std::time::Instant::now();
    let err = mssql::Client::connect(config, tcp.compat_write())
        .await
        .unwrap_err();
    assert!(is_timed_out(&err), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    Ok(())
}
