//! A connection left with a partly written request must refuse further use
//! instead of desyncing the server or hanging. Ported from upstream Tiberius
//! (tiberius-rs/tiberius 3e6afc3, 3f47d87).

use futures_util::FutureExt;
use mssql::{ColumnData, Config, TokenRow};
use once_cell::sync::Lazy;
use std::{env, time::Duration};
use tokio_util::compat::TokioAsyncWriteCompatExt;
use uuid::Uuid;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

type Client = mssql::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

async fn connect() -> mssql::Result<Client> {
    let config = Config::from_ado_string(&CONN_STR)?;
    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;
    mssql::Client::connect(config, tcp.compat_write()).await
}

// Poll a ~20 MB query exactly once and drop it: the first poll writes packets
// until the socket's send buffer fills and returns Pending, so the request is
// abandoned mid-message.
#[tokio::test]
async fn a_query_cancelled_mid_write_poisons_the_connection() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let huge = format!("SELECT 1 --{}", "x".repeat(20 * 1024 * 1024));

    let still_writing = conn.simple_query(huge).now_or_never().is_none();
    assert!(
        still_writing,
        "expected the 20 MB write to be still in progress after one poll"
    );

    let next = tokio::time::timeout(Duration::from_secs(10), async {
        conn.query("SELECT 1", &[]).await?.into_row().await
    })
    .await
    .expect("the next query hung instead of failing");

    match next {
        Err(mssql::error::Error::Protocol(msg)) => {
            assert!(msg.contains("partly written"), "{msg}")
        }
        other => panic!("expected a poisoned-connection error, got {other:?}"),
    }
    Ok(())
}

// Dropping a `BulkLoadRequest` between rows (no write in flight) is
// recoverable: the server ends the abandoned load on the next request, so
// it must not be treated as poisoned.
#[tokio::test]
async fn dropping_a_bulk_load_between_rows_leaves_the_connection_usable() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let table = format!("##t{}", Uuid::new_v4().simple());
    conn.simple_query(format!(
        "CREATE TABLE {table} (id INT NOT NULL, v NVARCHAR(4000) NOT NULL)"
    ))
    .await?
    .into_results()
    .await?;

    {
        let mut req = conn.bulk_insert(&table).await?;
        for id in 0..10 {
            let mut row = TokenRow::new();
            row.push(ColumnData::I32(Some(id)));
            row.push(ColumnData::String(Some("x".repeat(2000).into())));
            req.send(row).await?;
        }
    }

    let row = conn
        .query("SELECT 1", &[])
        .await?
        .into_row()
        .await?
        .unwrap();
    assert_eq!(Some(1i32), row.get(0));
    Ok(())
}
