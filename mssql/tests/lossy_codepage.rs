//! `Config::lossy_codepage_decoding` (ported from upstream Tiberius
//! 35605c9): an invalid byte sequence in a code-page column is an error by
//! default and U+FFFD when the option is on.

use mssql::Config;
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

async fn connect(database: Option<&str>, lossy: bool) -> mssql::Result<Client> {
    let mut config = Config::from_ado_string(&CONN_STR)?;
    if let Some(db) = database {
        config.database(db);
    }
    config.lossy_codepage_decoding(lossy);
    let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;
    mssql::Client::connect(config, tcp.compat_write()).await
}

// In a database whose default collation is Chinese_PRC_CI_AS (GBK), a
// varchar cast from 0x61 0x81 0x20 0x62 keeps those bytes, and 0x81 0x20 is
// not a valid GBK sequence (0x20 can't be a trail byte).
async fn read_malformed_gbk(lossy: bool) -> mssql::Result<Option<String>> {
    let database = format!("lossy_{}", Uuid::new_v4().simple());
    let mut admin = connect(None, false).await?;
    admin
        .simple_query(format!(
            "CREATE DATABASE [{database}] COLLATE Chinese_PRC_CI_AS"
        ))
        .await?
        .into_results()
        .await?;

    let result = async {
        let mut conn = connect(Some(&database), lossy).await?;
        let row = conn
            .simple_query("SELECT CAST(0x61812062 AS VARCHAR(4)), CAST(CAST(0x61812062 AS VARCHAR(4)) AS TEXT)")
            .await?
            .into_row()
            .await?
            .unwrap();
        let varchar = row.try_get::<&str, _>(0)?.map(str::to_owned);
        let text = row.try_get::<&str, _>(1)?.map(str::to_owned);
        assert_eq!(varchar, text, "varchar and text decode alike");
        Ok(varchar)
    }
    .await;

    let _ = admin
        .simple_query(format!("DROP DATABASE [{database}]"))
        .await?
        .into_results()
        .await;

    result
}

#[tokio::test]
async fn invalid_code_page_bytes_error_by_default() {
    let err = read_malformed_gbk(false).await.unwrap_err();
    assert!(matches!(err, mssql::error::Error::Encoding(_)), "{err:?}");
}

#[tokio::test]
async fn invalid_code_page_bytes_become_replacement_chars_when_lossy() -> mssql::Result<()> {
    let decoded = read_malformed_gbk(true).await?.unwrap();
    assert!(
        decoded.starts_with('a') && decoded.ends_with('b'),
        "{decoded:?}"
    );
    assert!(decoded.contains('\u{fffd}'), "{decoded:?}");
    Ok(())
}
