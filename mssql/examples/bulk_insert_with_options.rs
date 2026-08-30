//! Bulk-inserting into a subset of a table's columns, with
//! [`SqlBulkCopyOptions`] and an `ORDER` hint, mirroring .NET's
//! `SqlBulkCopy`.
use mssql::{Client, Config, IntoRow, SortOrder, SqlBulkCopyOption};
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

    client
        .execute(
            "IF OBJECT_ID('##bulk_options_test') IS NOT NULL DROP TABLE ##bulk_options_test",
            &[],
        )
        .await?;
    client
        .execute(
            "CREATE TABLE ##bulk_options_test (
                id INT NOT NULL,
                amount FLOAT NOT NULL,
                note VARCHAR(40) NULL
            )",
            &[],
        )
        .await?;

    // TABLOCK for throughput on a bulk load into an otherwise-idle table,
    // and an ORDER hint since we already know the rows arrive sorted by
    // `id` — this can speed up the load when the destination has a
    // matching index. `note` is deliberately left out of the column list;
    // it will be NULL for every inserted row.
    let options = SqlBulkCopyOption::TableLock | SqlBulkCopyOption::CheckConstraints;
    let order_hints = [("id", SortOrder::Ascending)];

    let mut req = client
        .bulk_insert_with_options(
            "##bulk_options_test",
            &["id", "amount"],
            options,
            &order_hints,
        )
        .await?;

    for i in 0..100i32 {
        req.send((i, i as f64 * 1.5).into_row()).await?;
    }

    let res = req.finalize().await?;
    println!("inserted {} rows", res.total());

    Ok(())
}
