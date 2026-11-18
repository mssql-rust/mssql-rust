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
async fn untyped_null_output_placeholder_is_rejected_client_side() -> mssql::Result<()> {
    let mut conn = connect().await?;

    // A NULL string is still sent untyped (NULLTYPE), so it can't be OUTPUT.
    let result = conn
        .call_procedure(
            "##does_not_matter",
            &[ProcParam::output("@out", &None::<&str>)],
        )
        .await;

    let err = result.expect_err("an untyped NULL OUTPUT placeholder must be rejected");
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

// NULLs of bit, the integer and float types, and uniqueidentifier are now
// sent typed, so they work as OUTPUT placeholders. Ported from upstream
// Tiberius 73e79d4.
#[tokio::test]
async fn typed_null_output_placeholders_work() -> mssql::Result<()> {
    let mut conn = connect().await?;
    let proc = random_proc_name();

    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@i INT OUTPUT, @b BIGINT OUTPUT, @f FLOAT OUTPUT, \
         @bit BIT OUTPUT, @g UNIQUEIDENTIFIER OUTPUT, @keep INT OUTPUT) AS \
         BEGIN SET @i = 1; SET @b = 2; SET @f = 0.5; SET @bit = 1; \
         SET @g = '6F9619FF-8B86-D011-B42D-00C04FC964FF'; \
         SET @keep = COALESCE(@keep, -1); END"
    ))
    .await?
    .into_results()
    .await?;

    let outputs = conn
        .call_procedure(
            proc.as_str(),
            &[
                ProcParam::output("@i", &None::<i32>),
                ProcParam::output("@b", &None::<i64>),
                ProcParam::output("@f", &None::<f64>),
                ProcParam::output("@bit", &None::<bool>),
                ProcParam::output("@g", &None::<Uuid>),
                // The NULL arrives as a real NULL, not a zero.
                ProcParam::output("@keep", &None::<i32>),
            ],
        )
        .await?
        .into_output_params()
        .await?;

    assert_eq!(Some(1i32), outputs.get("@i"));
    assert_eq!(Some(2i64), outputs.get("@b"));
    assert_eq!(Some(0.5f64), outputs.get("@f"));
    assert_eq!(Some(true), outputs.get("@bit"));
    assert_eq!(
        Some(Uuid::parse_str("6F9619FF-8B86-D011-B42D-00C04FC964FF").unwrap()),
        outputs.get("@g")
    );
    assert_eq!(Some(-1i32), outputs.get("@keep"));
    Ok(())
}

// Table-valued parameters (ported and reworked from upstream Tiberius
// 73e79d4, without its derive macro or unquoted type-name interpolation).
#[tokio::test]
async fn table_valued_parameter_round_trips() -> mssql::Result<()> {
    use mssql::{numeric::Numeric, ColumnData, IntoSql, TokenRow};

    let mut conn = connect().await?;
    let suffix = Uuid::new_v4().simple().to_string();
    let ty = format!("dbo.tvp_type_{suffix}");
    let proc = format!("dbo.tvp_proc_{suffix}");

    conn.simple_query(format!(
        "CREATE TYPE {ty} AS TABLE (id INT NOT NULL, name NVARCHAR(50) NULL, \
         amount DECIMAL(10,2) NULL, flag BIT NULL)"
    ))
    .await?
    .into_results()
    .await?;
    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@rows {ty} READONLY) AS \
         SELECT COUNT(*), SUM(id), MAX(name), SUM(amount), \
                SUM(CASE WHEN name IS NULL THEN 1 ELSE 0 END) FROM @rows"
    ))
    .await?
    .into_results()
    .await?;

    let result = async {
        let mut rows = Vec::new();
        for (id, name, amount) in [
            (1, Some("a"), Some(150)),
            (2, None, None),
            (3, Some("é"), Some(25)),
        ] {
            let mut row = TokenRow::new();
            row.push(ColumnData::I32(Some(id)));
            row.push(name.into_sql());
            row.push(ColumnData::Numeric(
                amount.map(|a| Numeric::new_with_scale(a, 2)),
            ));
            row.push(ColumnData::Bit(Some(id % 2 == 0)));
            rows.push(row);
        }

        let row = conn
            .call_procedure(
                proc.as_str(),
                &[ProcParam::table("@rows", ty.as_str(), rows)],
            )
            .await?
            .into_row()
            .await?
            .unwrap();
        assert_eq!(Some(3i32), row.get(0));
        assert_eq!(Some(6i32), row.get(1));
        assert_eq!(Some("é"), row.get::<&str, _>(2));
        let sum: Numeric = row.get(3).unwrap();
        assert_eq!((sum.value(), sum.scale()), (175, 2));
        assert_eq!(Some(1i32), row.get(4));

        // An empty table.
        let row = conn
            .call_procedure(
                proc.as_str(),
                &[ProcParam::table("@rows", ty.as_str(), vec![])],
            )
            .await?
            .into_row()
            .await?
            .unwrap();
        assert_eq!(Some(0i32), row.get(0));

        // A row of the wrong width is a client-side error.
        let mut short = TokenRow::new();
        short.push(ColumnData::I32(Some(1)));
        assert!(conn
            .call_procedure(
                proc.as_str(),
                &[ProcParam::table("@rows", ty.as_str(), vec![short])]
            )
            .await
            .is_err());

        // An injection-shaped type name is rejected before any SQL is built.
        let err = conn
            .call_procedure(
                proc.as_str(),
                &[ProcParam::table("@rows", "dbo.t; DROP TABLE x--", vec![])],
            )
            .await
            .unwrap_err();
        assert!(!matches!(err, mssql::error::Error::Server(_)), "{err:?}");

        // The connection is still usable.
        let row = conn
            .simple_query("SELECT 1")
            .await?
            .into_row()
            .await?
            .unwrap();
        assert_eq!(Some(1i32), row.get(0));
        Ok::<_, mssql::error::Error>(())
    }
    .await;

    conn.simple_query(format!("DROP PROCEDURE {proc}; DROP TYPE {ty}"))
        .await?
        .into_results()
        .await?;
    result
}

#[cfg(all(feature = "jiff", feature = "tds73"))]
#[tokio::test]
async fn table_valued_parameter_with_more_column_types() -> mssql::Result<()> {
    use mssql::{ColumnData, IntoSql, TokenRow};

    let mut conn = connect().await?;
    let suffix = Uuid::new_v4().simple().to_string();
    let ty = format!("[dbo].[tvp wide {suffix}]");
    let proc = format!("dbo.tvp_wide_{suffix}");

    conn.simple_query(format!(
        "CREATE TYPE {ty} AS TABLE (b BIGINT, f FLOAT, m MONEY, g UNIQUEIDENTIFIER, \
         dt DATETIME, d DATE, d2 DATETIME2, v VARCHAR(20))"
    ))
    .await?
    .into_results()
    .await?;
    conn.simple_query(format!(
        "CREATE PROCEDURE {proc} (@rows {ty} READONLY) AS \
         SELECT b, f, CAST(m AS VARCHAR(30)), g, CONVERT(VARCHAR(30), dt, 126), \
                CONVERT(VARCHAR(10), d, 23), CONVERT(VARCHAR(30), d2, 126), v FROM @rows"
    ))
    .await?
    .into_results()
    .await?;

    let result = async {
        let g = Uuid::parse_str("6F9619FF-8B86-D011-B42D-00C04FC964FF").unwrap();
        let mut row = TokenRow::new();
        row.push(ColumnData::I64(Some(9_000_000_000)));
        row.push(ColumnData::F64(Some(2.5)));
        row.push(ColumnData::F64(Some(12.34)));
        row.push(ColumnData::Guid(Some(g)));
        row.push(ColumnData::DateTime(None)); // a NULL datetime
        row.push(jiff::civil::date(2020, 4, 20).into_sql());
        row.push(jiff::civil::date(2020, 4, 20).at(16, 20, 5, 0).into_sql());
        row.push("plain".into_sql());

        let row = conn
            .call_procedure(
                proc.as_str(),
                &[ProcParam::table("@rows", ty.as_str(), vec![row])],
            )
            .await?
            .into_row()
            .await?
            .unwrap();
        assert_eq!(Some(9_000_000_000i64), row.get(0));
        assert_eq!(Some(2.5f64), row.get(1));
        assert_eq!(Some("12.34"), row.get::<&str, _>(2));
        assert_eq!(Some(g), row.get::<Uuid, _>(3));
        assert_eq!(None, row.get::<&str, _>(4));
        assert_eq!(Some("2020-04-20"), row.get::<&str, _>(5));
        assert_eq!(Some("2020-04-20T16:20:05"), row.get::<&str, _>(6));
        assert_eq!(Some("plain"), row.get::<&str, _>(7));
        Ok::<_, mssql::error::Error>(())
    }
    .await;

    conn.simple_query(format!("DROP PROCEDURE {proc}; DROP TYPE {ty}"))
        .await?
        .into_results()
        .await?;
    result
}
