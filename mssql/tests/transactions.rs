//! Transaction Manager requests (begin/commit/rollback/savepoint), ported
//! from upstream Tiberius (tiberius-rs/tiberius 8338bb8, 0965507).

use mssql::{Config, IsolationLevel};
use once_cell::sync::Lazy;
use std::env;
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

async fn scalar(conn: &mut Client, sql: &str) -> mssql::Result<i32> {
    let row = conn.simple_query(sql).await?.into_row().await?.unwrap();
    Ok(row.get::<i32, _>(0).unwrap())
}

async fn new_table(conn: &mut Client) -> mssql::Result<String> {
    let table = format!("##t{}", Uuid::new_v4().simple());
    conn.simple_query(format!("CREATE TABLE {table} (id INT NOT NULL)"))
        .await?
        .into_results()
        .await?;
    Ok(table)
}

#[tokio::test]
async fn commit_keeps_and_rollback_discards() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let table = new_table(&mut conn).await?;

    conn.begin_transaction().await?;
    assert_eq!(1, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);
    conn.execute(format!("INSERT INTO {table} VALUES (1)"), &[])
        .await?;
    conn.rollback_transaction().await?;
    assert_eq!(0, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);
    assert_eq!(
        0,
        scalar(&mut conn, &format!("SELECT COUNT(*) FROM {table}")).await?
    );

    conn.begin_transaction().await?;
    conn.execute(format!("INSERT INTO {table} VALUES (2)"), &[])
        .await?;
    conn.commit_transaction().await?;
    assert_eq!(0, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);
    assert_eq!(
        1,
        scalar(&mut conn, &format!("SELECT COUNT(*) FROM {table}")).await?
    );
    Ok(())
}

#[tokio::test]
async fn savepoint_rollback_keeps_earlier_work() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let table = new_table(&mut conn).await?;

    conn.begin_transaction().await?;
    conn.execute(format!("INSERT INTO {table} VALUES (1)"), &[])
        .await?;
    conn.save_transaction("sp1").await?;
    conn.execute(format!("INSERT INTO {table} VALUES (2)"), &[])
        .await?;
    conn.rollback_to_savepoint("sp1").await?;
    assert_eq!(1, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);
    conn.commit_transaction().await?;

    assert_eq!(
        1,
        scalar(&mut conn, &format!("SELECT COUNT(*) FROM {table}")).await?
    );
    Ok(())
}

#[tokio::test]
async fn isolation_level_is_applied() -> mssql::Result<()> {
    let mut conn = connect().await?;
    conn.begin_transaction_with_isolation(IsolationLevel::Serializable)
        .await?;
    // sys.dm_exec_sessions: 4 = SERIALIZABLE.
    let level = scalar(
        &mut conn,
        "SELECT CAST(transaction_isolation_level AS INT) FROM sys.dm_exec_sessions WHERE session_id = @@SPID",
    )
    .await?;
    conn.rollback_transaction().await?;
    assert_eq!(4, level);
    Ok(())
}

#[tokio::test]
async fn commit_without_a_transaction_is_a_server_error() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let err = conn.commit_transaction().await.unwrap_err();
    assert!(matches!(err, mssql::error::Error::Server(_)), "{err:?}");
    // The connection is still usable.
    assert_eq!(1, scalar(&mut conn, "SELECT 1").await?);
    Ok(())
}
