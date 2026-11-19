use mssql::Config;
use once_cell::sync::Lazy;
use std::env;
use tokio_util::compat::TokioAsyncWriteCompatExt;

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

#[tokio::test]
async fn reset_clears_session_settings_and_rolls_back_transactions() -> mssql::Result<()> {
    let mut conn = connect().await?;

    conn.simple_query("SET ROWCOUNT 1; BEGIN TRANSACTION")
        .await?
        .into_results()
        .await?;
    assert_eq!(1, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);

    conn.reset().await?;

    assert_eq!(0, scalar(&mut conn, "SELECT @@TRANCOUNT").await?);
    let rows = conn
        .simple_query("SELECT value FROM (VALUES (1), (2)) AS values_(value) ORDER BY value")
        .await?
        .into_first_result()
        .await?;
    assert_eq!(2, rows.len(), "SET ROWCOUNT must be restored by reset");

    Ok(())
}
