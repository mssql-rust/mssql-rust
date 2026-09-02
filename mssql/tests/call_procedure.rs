//! Regression tests for prisma/tiberius#275: stored-procedure `OUTPUT`
//! parameters were decoded but silently discarded, and nothing in the
//! public API could ever cause the server to send one in the first place
//! (`Client::execute`/`query` always go through `sp_executesql`, whose own
//! parameters can't be bound `OUTPUT`; the RPC-by-name path a real `OUTPUT`
//! call needs was a literal `todo!()`).

use mssql::{Config, ProcParam};
use once_cell::sync::Lazy;
use std::env;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use uuid::Uuid;

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

fn random_proc_name() -> String {
    format!("##proc_{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn single_output_param_and_return_status() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let proc = random_proc_name();

    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@n INT, @doubled INT OUTPUT) AS
         BEGIN SET @doubled = @n * 2; RETURN 1; END"
    ))
    .await?;

    let stream = conn
        .call_procedure(
            proc.as_str(),
            &[
                ProcParam::input("@n", &21i32),
                ProcParam::output("@doubled", &0i32),
            ],
        )
        .await?;

    let outputs = stream.into_output_params().await?;

    assert_eq!(Some(42i32), outputs.get("@doubled"));
    // The name may be given without its leading `@` too.
    assert_eq!(Some(42i32), outputs.get("doubled"));
    assert_eq!(Some(1), outputs.return_status());

    Ok(())
}

#[tokio::test]
async fn multiple_output_params() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let proc = random_proc_name();

    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@a INT, @b INT, @sum INT OUTPUT, @product INT OUTPUT) AS
         BEGIN SET @sum = @a + @b; SET @product = @a * @b; END"
    ))
    .await?;

    let stream = conn
        .call_procedure(
            proc.as_str(),
            &[
                ProcParam::input("@a", &6i32),
                ProcParam::input("@b", &7i32),
                ProcParam::output("@sum", &0i32),
                ProcParam::output("@product", &0i32),
            ],
        )
        .await?;

    let outputs = stream.into_output_params().await?;

    assert_eq!(Some(13i32), outputs.get("@sum"));
    assert_eq!(Some(42i32), outputs.get("@product"));
    // No RETURN statement in this procedure - SQL Server still sends an
    // implicit RETURNSTATUS of 0 for any RPC call, unlike a plain query
    // (see `into_output_params_on_a_plain_query_is_empty`, which doesn't
    // go through the RPC path at all).
    assert_eq!(Some(0), outputs.return_status());

    Ok(())
}

#[tokio::test]
async fn result_set_rows_before_output_params_are_still_readable() -> mssql::Result<()> {
    use futures_util::stream::TryStreamExt;
    use mssql::QueryItem;

    let mut conn = connect().await?;
    let proc = random_proc_name();

    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@n INT, @doubled INT OUTPUT) AS
         BEGIN SELECT @n AS original; SET @doubled = @n * 2; END"
    ))
    .await?;

    let mut stream = conn
        .call_procedure(
            proc.as_str(),
            &[
                ProcParam::input("@n", &21i32),
                ProcParam::output("@doubled", &0i32),
            ],
        )
        .await?;

    let mut rows = Vec::new();
    while let Some(item) = stream.try_next().await? {
        if let QueryItem::Row(row) = item {
            rows.push(row.get::<i32, _>("original"));
        }
    }
    assert_eq!(vec![Some(21i32)], rows);

    let outputs = stream.into_output_params().await?;
    assert_eq!(Some(42i32), outputs.get("@doubled"));

    Ok(())
}

// A NULL placeholder can't carry a type on its own for a direct RPC call
// (unlike `execute`/`query`'s separate `sp_executesql` type-declaration
// string) - SQL Server rejects it, so this is checked client-side with a
// clear error instead of round-tripping to a cryptic server one. Doesn't
// need the procedure to actually exist - the check happens before anything
// is sent.
#[tokio::test]
async fn null_output_placeholder_is_rejected_client_side() -> mssql::Result<()> {
    let mut conn = connect().await?;

    let result = conn
        .call_procedure(
            "##does_not_matter",
            &[ProcParam::output("@out", &None::<i32>)],
        )
        .await;

    let err = result.expect_err("a NULL OUTPUT placeholder must be rejected");
    assert!(
        matches!(err, mssql::error::Error::Conversion(_)),
        "expected Error::Conversion, got {err:?}"
    );
    assert!(err.to_string().contains("@out"));

    Ok(())
}

#[tokio::test]
async fn into_output_params_on_a_plain_query_is_empty() -> mssql::Result<()> {
    let mut conn = connect().await?;

    let outputs = conn
        .simple_query("SELECT 1 AS col")
        .await?
        .into_output_params()
        .await?;

    assert_eq!(None, outputs.return_status());
    assert!(outputs.try_get::<i32>("@anything").is_err());

    Ok(())
}
