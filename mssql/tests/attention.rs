//! Regression tests for prisma/tiberius#300 / prisma/tiberius#79:
//! cancelling (dropping) an in-flight query used to leave the connection
//! unusable for however long the abandoned request would otherwise have
//! taken to finish server-side - no TDS Attention (cancel) signal was ever
//! sent, so the next query on the same connection just waited behind it.

use mssql::Config;
use once_cell::sync::Lazy;
use std::env;
use std::time::{Duration, Instant};
use tokio_util::compat::TokioAsyncWriteCompatExt;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

async fn connect() -> mssql::Result<mssql::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>>
{
    let config = Config::from_ado_string(&CONN_STR)?;
    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;
    mssql::Client::connect(config, tcp.compat_write()).await
}

// The exact scenario from prisma/tiberius#300: wrap a still-executing
// query in `tokio::time::timeout`, then issue another query on the same
// connection. Before this fix, the second query would block for as long
// as the first one would have taken to finish on its own (here, up to 10
// seconds) instead of completing promptly.
#[tokio::test]
async fn cancelling_a_still_executing_query_does_not_hang_the_connection() -> mssql::Result<()> {
    let mut conn = connect().await?;

    let cancelled = tokio::time::timeout(
        Duration::from_secs(1),
        conn.simple_query("WAITFOR DELAY '00:00:10'"),
    )
    .await;

    assert!(
        cancelled.is_err(),
        "expected the 1s timeout to fire before the 10s WAITFOR finished"
    );
    drop(cancelled);

    let start = Instant::now();

    let row = conn
        .simple_query("SELECT 1 AS col")
        .await?
        .into_row()
        .await?
        .unwrap();

    assert_eq!(Some(1i32), row.get("col"));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "next query took {:?} - the cancelled query's connection wasn't cleaned up",
        start.elapsed()
    );

    Ok(())
}

// A caller that just doesn't read every row of a query that *did* fully
// complete (no cancellation involved) must keep working exactly as before
// - `flush_stream` now unconditionally sends Attention when the stream is
// dirty, and this is the scenario where the server has nothing in flight
// to cancel, so the fix must not regress it.
#[tokio::test]
async fn dropping_a_partially_read_but_completed_stream_does_not_hang_the_connection(
) -> mssql::Result<()> {
    let mut conn = connect().await?;

    {
        // Issued, but never read: by the time this block ends the
        // response has almost certainly already fully arrived.
        let _stream = conn
            .simple_query("SELECT 1 AS col UNION ALL SELECT 2 UNION ALL SELECT 3")
            .await?;

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let start = Instant::now();

    let row = conn
        .simple_query("SELECT 42 AS col")
        .await?
        .into_row()
        .await?
        .unwrap();

    assert_eq!(Some(42i32), row.get("col"));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "next query took {:?}",
        start.elapsed()
    );

    Ok(())
}
