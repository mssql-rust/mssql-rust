//! Building a SQL `IN (...)` list.
//!
//! SQL Server has no array parameter, so an `IN` list must name one
//! placeholder per value — binding a comma-separated string to `IN (@P1)`
//! matches nothing rather than failing. [`Query::placeholders`] builds the
//! placeholder list, [`Query::bind_iter`] binds each value in order, and
//! [`Query::MAX_PARAMETERS`] is the server's 2100-parameter-per-statement
//! limit (relevant for a long `IN` list or a multi-row `INSERT`, since it's
//! reached by data volume rather than something a fixed set of tests would
//! catch, and the server only reports it after the whole batch is sent).
use mssql::{Client, Config, Query};
use once_cell::sync::Lazy;
use std::env;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_ado_string(&CONN_STR)?;

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let ids = [1i32, 2, 3, 5, 8];

    let sql = format!(
        "SELECT value FROM (VALUES (1), (2), (3), (4), (5), (6), (7), (8)) AS t(value) \
         WHERE value IN ({})",
        Query::placeholders(1, ids.len())
    );

    let mut query = Query::new(sql);
    query.bind_iter(ids);
    assert_eq!(ids.len(), query.param_count());

    let stream = query.query(&mut client).await?;
    let rows = stream.into_first_result().await?;

    println!("matched {} of 8 candidate rows", rows.len());
    assert_eq!(ids.len(), rows.len());

    Ok(())
}
