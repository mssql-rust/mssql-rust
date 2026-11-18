//! A real TDS 8.0 Strict login, against a server that supports it (SQL
//! Server 2025, or 2022 / Azure SQL with a strict-capable certificate).
//! Opt-in: set MSSQL_TEST_STRICT_CONNECTION_STRING to a connection string
//! with `encrypt=strict`; otherwise the test does nothing. Azure SQL Edge,
//! the default test server, doesn't speak TDS 8.0.

use mssql::Config;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::test]
async fn strict_login_uses_tds_8() -> mssql::Result<()> {
    let Ok(conn_str) = std::env::var("MSSQL_TEST_STRICT_CONNECTION_STRING") else {
        eprintln!("skipping: MSSQL_TEST_STRICT_CONNECTION_STRING is not set");
        return Ok(());
    };

    let config = Config::from_ado_string(&conn_str)?;
    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;

    let row = client
        .simple_query(
            "SELECT CONVERT(VARCHAR(20), CONVERT(VARBINARY(4), protocol_version), 1), encrypt_option \
             FROM sys.dm_exec_connections WHERE session_id = @@SPID",
        )
        .await?
        .into_row()
        .await?
        .unwrap();

    assert_eq!(Some("0x08000000"), row.get::<&str, _>(0), "TDS 8.0");
    assert_eq!(Some("TRUE"), row.get::<&str, _>(1));
    Ok(())
}
