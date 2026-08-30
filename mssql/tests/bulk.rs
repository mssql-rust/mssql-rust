use futures_util::io::{AsyncRead, AsyncWrite};
#[cfg(all(feature = "tds73", feature = "time"))]
use mssql::ToSql;
use mssql::{ColumnData, IntoSql, Result, SortOrder, SqlBulkCopyOption, TokenRow};
use once_cell::sync::Lazy;
use std::env;
use std::sync::Once;
use uuid::Uuid;

#[cfg(all(feature = "tds73", feature = "chrono"))]
use chrono::DateTime;
#[cfg(all(feature = "tds73", feature = "chrono"))]
use chrono::NaiveDateTime;

use runtimes_macro::test_on_runtimes;

// This is used in the testing macro :)
#[allow(dead_code)]
static LOGGER_SETUP: Once = Once::new();

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

async fn random_table() -> String {
    format!("t{}", Uuid::new_v4().simple())
}

macro_rules! test_bulk_type {
    ($name:ident($sql_type:literal, $total_generated:expr, $generator:expr)) => {
        pastey::item! {
            #[test_on_runtimes]
            async fn [< bulk_load_optional_ $name >]<S>(mut conn: mssql::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                let table = format!("##{}", random_table().await);

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, content {} NULL)",
                        table,
                        $sql_type,
                    ),
                    &[],
                )
                    .await?;

                let mut req = conn.bulk_insert(&table).await?;

                for i in $generator {
                    let mut row = TokenRow::new();
                    row.push(i.into_sql());
                    req.send(row).await?;
                }

                let res = req.finalize().await?;

                assert_eq!($total_generated, res.total());

                Ok(())
            }

            #[test_on_runtimes]
            async fn [< bulk_load_required_ $name >]<S>(mut conn: mssql::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                let table = format!("##{}", random_table().await);

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, content {} NOT NULL)",
                        table,
                        $sql_type
                    ),
                    &[],
                )
                    .await?;

                let mut req = conn.bulk_insert(&table).await?;

                for i in $generator {
                    let mut row = TokenRow::new();
                    row.push(i.into_sql());
                    req.send(row).await?;
                }

                let res = req.finalize().await?;

                assert_eq!($total_generated, res.total());

                Ok(())
            }
        }
    };
}

test_bulk_type!(tinyint("TINYINT", 256, 0..=255u8));
test_bulk_type!(smallint("SMALLINT", 2000, 0..2000i16));
test_bulk_type!(int("INT", 2000, 0..2000i32));
test_bulk_type!(bigint("BIGINT", 2000, 0..2000i64));

test_bulk_type!(empty_varchar(
    "VARCHAR(MAX)",
    100,
    vec![""; 100].into_iter()
));
test_bulk_type!(empty_nvarchar(
    "NVARCHAR(MAX)",
    100,
    vec![""; 100].into_iter()
));
test_bulk_type!(empty_varbinary(
    "VARBINARY(MAX)",
    100,
    vec![b""; 100].into_iter()
));

// Regression test for #322: bulk-inserting a large (well past the 8000-byte
// non-MAX limit) value into a MAX-typed column was reported to fail
// server-side with error 4816. The existing empty_varchar/empty_nvarchar/
// empty_varbinary tests above only cover the zero-length case (#315's
// original fix scope) - nothing exercised an actual large value.
test_bulk_type!(large_varchar(
    "VARCHAR(MAX)",
    5,
    vec!["a".repeat(80_000); 5].into_iter()
));
test_bulk_type!(large_nvarchar(
    "NVARCHAR(MAX)",
    5,
    vec!["a".repeat(80_000); 5].into_iter()
));
test_bulk_type!(large_varbinary(
    "VARBINARY(MAX)",
    5,
    vec![vec![0xABu8; 80_000]; 5].into_iter()
));

test_bulk_type!(real(
    "REAL",
    1000,
    vec![std::f32::consts::PI; 1000].into_iter()
));

test_bulk_type!(float(
    "FLOAT",
    1000,
    vec![std::f64::consts::PI; 1000].into_iter()
));

test_bulk_type!(varchar_limited(
    "VARCHAR(255)",
    1000,
    vec!["aaaaaaaaaaaaaaaaaaaaaaa"; 1000].into_iter()
));

// Regression test for prisma/tiberius#376: `Decimal` had `ToSql` (borrowed)
// but no `IntoSql` (owned) impl, so it couldn't be bulk-inserted via
// `row.push(value.into_sql())` the way every other supported type can.
#[cfg(all(feature = "tds73", feature = "rust_decimal"))]
test_bulk_type!(decimal(
    "DECIMAL(20, 4)",
    100,
    vec![mssql::numeric::Decimal::from_i128_with_scale(123456789, 4); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2(
    "DATETIME2",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_naive("DATETIME2", 100, {
    #[allow(deprecated)]
    let dt = NaiveDateTime::from_timestamp_opt(1658524194, 123456789).unwrap();

    vec![dt; 100].into_iter()
}));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_0(
    "DATETIME2(0)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_1(
    "DATETIME2(1)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_2(
    "DATETIME2(2)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_3(
    "DATETIME2(3)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_4(
    "DATETIME2(4)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_5(
    "DATETIME2(5)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_6(
    "DATETIME2(6)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_7(
    "DATETIME2(7)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

macro_rules! test_bulk_columns {
    ($name:ident($total_generated:literal $(, $sql_type:literal)+ $(, ($cols:expr, $generator:expr ))+ $(,)?)) => {
        pastey::item! {
            #[test_on_runtimes]
            async fn [< bulk_load_optional_ $name >]<S>(mut conn: mssql::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                use mssql::IntoRow;

                let table = format!("##{}", random_table().await);
                let column_defs = &[$($sql_type,)+];

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, {})",
                        table,
                        column_defs.join(", "),
                    ),
                    &[],
                )
                    .await?;

                let mut count = 0;

                $(
                    let mut req = conn.bulk_insert_columns(&table, $cols).await?;
                    for i in $generator {
                        let row = i.into_row();
                        req.send(row).await?;
                    }

                    let res = req.finalize().await?;
                    count += res.total();
                )+
                assert_eq!($total_generated, count);

                Ok(())
            }

            #[test_on_runtimes]
            async fn [< bulk_load_required_ $name >]<S>(mut conn: mssql::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                use mssql::IntoRow;
                let table = format!("##{}", random_table().await);
                let column_defs = &[$(format!("{} NOT NULL", $sql_type),)+];

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, {})",
                        table,
                        column_defs.join(", "),
                    ),
                    &[],
                )
                    .await?;

                let mut count = 0;

                $(
                    let mut req = conn.bulk_insert_columns(&table, $cols).await?;
                    for i in $generator {
                        let row = i.into_row();
                        req.send(row).await?;
                    }

                    let res = req.finalize().await?;
                    count += res.total();
                )+
                assert_eq!($total_generated, count);

                Ok(())
            }
        }
    };
}

test_bulk_columns!(ab_ba_default_columns(
    200,
    "a INT",
    "b FLOAT",
    "c INT DEFAULT 0",
    (&["a", "b"], vec![(1i32, 1f64); 100]),
    (&["b", "a"], vec![(2f64, 2i32); 100]),
));

test_bulk_columns!(ab_ba_override_default_columns(
    200,
    "a INT",
    "b FLOAT",
    "c INT DEFAULT 0",
    (&["a", "b", "c"], vec![(1i32, 1f64, 10i32); 100]),
    (&["b", "c", "a"], vec![(2f64, 20i32, 2i32); 100]),
));

// Regression test for prisma/tiberius#296 / #387 / #388: a column named
// after a reserved word (or containing a space) used to break bulk_insert's
// generated INSERT BULK statement text, since the column name wasn't
// bracket-quoted.
#[test_on_runtimes]
async fn read_and_write_to_keyword_columns<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!("CREATE TABLE {} ([End] INT)", table))
        .await?;

    let mut req = conn.bulk_insert(&table).await?;
    for num in [6, 7, 8] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(num)));
        req.send(row).await?;
    }
    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[3]);

    let rows = conn
        .query(format!("SELECT [End] FROM {}", table), &[])
        .await?
        .into_first_result()
        .await?;

    assert_eq!(rows.len(), 3);
    assert_eq!(Some(6), rows[0].get(0));
    assert_eq!(Some(7), rows[1].get(0));
    assert_eq!(Some(8), rows[2].get(0));

    Ok(())
}

// Regression test for #373/#410: DATE's TYPE_INFO wrongly wrote a
// length/scale byte it doesn't have, shifting every subsequent column's
// TYPE_INFO in the same COLMETADATA token by one byte and corrupting
// bulk_insert for any row where DATE wasn't the last column. A single-column
// DATE bulk insert (like the `test_bulk_type!` macro above generates) can't
// catch this - it only manifests when something follows.
#[cfg(all(feature = "tds73", feature = "time"))]
#[test_on_runtimes]
async fn bulk_insert_date_before_time_column<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.execute(
        &format!(
            "CREATE TABLE {} (id BIGINT, a_date DATE, a_time TIME(7))",
            table
        ),
        &[],
    )
    .await?;

    let date = time::Date::from_calendar_date(2024, time::Month::June, 15).unwrap();
    let clock = time::Time::from_hms(13, 45, 30).unwrap();

    let mut req = conn.bulk_insert(&table).await?;
    let mut row = TokenRow::new();
    row.push(1i64.into_sql());
    row.push(date.to_sql());
    row.push(clock.to_sql());
    req.send(row).await?;

    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[1]);

    let rows = conn
        .query(format!("SELECT id, a_date, a_time FROM {}", table), &[])
        .await?
        .into_first_result()
        .await?;

    assert_eq!(1, rows.len());
    assert_eq!(Some(1i64), rows[0].get(0));
    assert_eq!(Some(date), rows[0].get(1));
    assert_eq!(Some(clock), rows[0].get(2));

    Ok(())
}

// Regression test for the ColumnFlag::Updateable/UpdateableUnknown bit
// values (MS-TDS 2.2.7.4 COLMETADATA Flags, `usUpdateable`): identity and
// computed columns must be excluded from a wildcard bulk_insert's generated
// column list, while ordinary read/write columns (which SQL Server commonly
// reports as updateable-unknown rather than definitively read/write on a
// plain SELECT) must still be included.
#[test_on_runtimes]
async fn bulk_insert_excludes_identity_and_computed_columns<S>(
    mut conn: mssql::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (
            id INT IDENTITY PRIMARY KEY,
            content INT NULL,
            doubled AS (content * 2)
        )",
        table
    ))
    .await?;

    let mut req = conn.bulk_insert(&table).await?;
    for num in [1, 2, 3] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(num)));
        req.send(row).await?;
    }
    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[3]);

    let rows = conn
        .query(
            format!("SELECT content, doubled FROM {} ORDER BY id", table),
            &[],
        )
        .await?
        .into_first_result()
        .await?;

    assert_eq!(rows.len(), 3);
    assert_eq!(Some(1), rows[0].get(0));
    assert_eq!(Some(2), rows[0].get(1));
    assert_eq!(Some(3), rows[2].get(0));
    assert_eq!(Some(6), rows[2].get(1));

    Ok(())
}

// Regression tests for prisma/tiberius#312 (SqlBulkCopyOptions / ORDER hint).

#[test_on_runtimes]
async fn bulk_insert_with_options_keep_identity<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (id INT IDENTITY(10, 1) PRIMARY KEY, val INT NOT NULL)",
        table
    ))
    .await?;

    let mut req = conn
        .bulk_insert_with_options(
            &table,
            &["id", "val"],
            SqlBulkCopyOption::KeepIdentity.into(),
            &[],
        )
        .await?;

    for (id, val) in [(1i32, 100i32), (2i32, 200i32)] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(id)));
        row.push(ColumnData::I32(Some(val)));
        req.send(row).await?;
    }
    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[2]);

    let rows = conn
        .query(format!("SELECT id, val FROM {} ORDER BY id", table), &[])
        .await?
        .into_first_result()
        .await?;

    assert_eq!(rows.len(), 2);
    // The identity values we supplied must have been preserved, not
    // reassigned starting from the seed (10).
    assert_eq!(Some(1), rows[0].get(0));
    assert_eq!(Some(100), rows[0].get(1));
    assert_eq!(Some(2), rows[1].get(0));
    assert_eq!(Some(200), rows[1].get(1));

    Ok(())
}

#[test_on_runtimes]
async fn bulk_insert_without_keep_identity_still_excludes_identity_column<S>(
    mut conn: mssql::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (id INT IDENTITY(10, 1) PRIMARY KEY, val INT NOT NULL)",
        table
    ))
    .await?;

    // Wildcard bulk_insert, no KeepIdentity: the identity column must be
    // excluded from the generated column list (same as
    // bulk_insert_excludes_identity_and_computed_columns), so only `val`
    // is supplied and the server assigns `id` starting from its seed.
    let mut req = conn.bulk_insert(&table).await?;
    for val in [100, 200] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(val)));
        req.send(row).await?;
    }
    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[2]);

    let rows = conn
        .query(format!("SELECT id, val FROM {} ORDER BY id", table), &[])
        .await?
        .into_first_result()
        .await?;

    assert_eq!(rows.len(), 2);
    assert_eq!(Some(10), rows[0].get(0));
    assert_eq!(Some(100), rows[0].get(1));
    assert_eq!(Some(11), rows[1].get(0));
    assert_eq!(Some(200), rows[1].get(1));

    Ok(())
}

#[test_on_runtimes]
async fn bulk_insert_with_options_table_lock_and_order_hint<S>(
    mut conn: mssql::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!("CREATE TABLE {} (val INT NOT NULL)", table))
        .await?;

    let options = SqlBulkCopyOption::TableLock
        | SqlBulkCopyOption::CheckConstraints
        | SqlBulkCopyOption::FireTriggers
        | SqlBulkCopyOption::KeepNulls;
    let order_hints = [("val", SortOrder::Ascending)];

    let mut req = conn
        .bulk_insert_with_options(&table, &["*"], options, &order_hints)
        .await?;

    for val in [3, 1, 2] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(val)));
        req.send(row).await?;
    }
    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[3]);

    let rows = conn
        .query(format!("SELECT val FROM {}", table), &[])
        .await?
        .into_first_result()
        .await?;

    assert_eq!(rows.len(), 3);

    Ok(())
}
