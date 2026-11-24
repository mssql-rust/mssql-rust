use futures_util::io::{AsyncRead, AsyncWrite};
#[cfg(all(feature = "tds73", feature = "time"))]
use mssql::ToSql;
use mssql::{ColumnData, IntoSql, Result, SortOrder, SqlBulkCopyOption, TokenRow};
use once_cell::sync::Lazy;
use std::env;
use std::sync::Once;
use uuid::Uuid;

#[cfg(all(feature = "tds73", feature = "jiff"))]
use jiff::civil::DateTime as CivilDateTime;
#[cfg(all(feature = "tds73", feature = "jiff"))]
use jiff::Timestamp;

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

                assert_eq!($total_generated, res.total()?);

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

                assert_eq!($total_generated, res.total()?);

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

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2(
    "DATETIME2",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_naive("DATETIME2", 100, {
    let dt: CivilDateTime = Timestamp::new(1658524194, 123456789)
        .unwrap()
        .to_zoned_datetime(jiff::tz::TimeZone::UTC)
        .datetime();

    vec![dt; 100].into_iter()
}));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_0(
    "DATETIME2(0)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_1(
    "DATETIME2(1)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_2(
    "DATETIME2(2)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_3(
    "DATETIME2(3)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_4(
    "DATETIME2(4)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_5(
    "DATETIME2(5)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_6(
    "DATETIME2(6)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "jiff"))]
test_bulk_type!(datetime2_7(
    "DATETIME2(7)",
    100,
    vec![Timestamp::new(1658524194, 123456789).unwrap(); 100].into_iter()
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
                    count += res.total()?;
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
                    count += res.total()?;
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

// A row whose encoding fails partway (here the second column, a smallmoney
// out of range) must not leave its first column's bytes in the bulk-load
// buffer: they used to be flushed with the next row and desync the stream.
// Ported from upstream Tiberius 991702f.
#[test_on_runtimes]
async fn bulk_insert_failed_row_is_rolled_back<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (id INT NOT NULL, m SMALLMONEY NULL)",
        table
    ))
    .await?;

    let mut req = conn.bulk_insert(&table).await?;

    let mut bad = TokenRow::new();
    bad.push(ColumnData::I32(Some(1)));
    bad.push(ColumnData::F64(Some(1e12)));
    assert!(req.send(bad).await.is_err());

    let mut good = TokenRow::new();
    good.push(ColumnData::I32(Some(2)));
    good.push(ColumnData::F64(Some(1.5)));
    req.send(good).await?;

    let result = req.finalize().await?;
    assert_eq!(result.rows_affected(), &[1]);

    let row = conn
        .query(format!("SELECT id, CAST(m AS FLOAT) FROM {}", table), &[])
        .await?
        .into_row()
        .await?
        .unwrap();
    assert_eq!(Some(2), row.get::<i32, _>(0));
    assert_eq!(Some(1.5), row.get::<f64, _>(1));

    Ok(())
}

// An ORDER hint naming a column with `]` in it must be bracket-escaped like
// every other identifier in the generated INSERT BULK statement.
#[test_on_runtimes]
async fn bulk_insert_order_hint_escapes_closing_bracket<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!("CREATE TABLE {} ([a]]b] INT NOT NULL)", table))
        .await?;

    let order_hints = [("a]b", SortOrder::Ascending)];
    let mut req = conn
        .bulk_insert_with_options(
            &table,
            &["*"],
            SqlBulkCopyOption::TableLock.into(),
            &order_hints,
        )
        .await?;

    for val in [1, 2] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(val)));
        req.send(row).await?;
    }
    assert_eq!(req.finalize().await?.rows_affected(), &[2]);

    Ok(())
}

// INSERT BULK must declare each character column's collation, or text bulk-
// loaded into a non-default-collation column is stored corrupted. Ported
// from upstream Tiberius (tiberius-rs/tiberius 75d7afc).
/// Bulk-insert "Привет" into Cyrillic_General_CI_AS (CP1251) columns of
/// `table` and check the stored bytes. The database default collation must
/// not use CP1251 for this to detect a missing `COLLATE` in `INSERT BULK`:
/// the server would then read the CP1251 bytes in the default code page.
async fn bulk_cyrillic_roundtrip<S>(conn: &mut mssql::Client<S>, table: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let default_code_page = conn
        .query(
            "SELECT CONVERT(INT, COLLATIONPROPERTY(CONVERT(NVARCHAR(128), \
             DATABASEPROPERTYEX(DB_NAME(), 'Collation')), 'CodePage'))",
            &[],
        )
        .await?
        .into_row()
        .await?
        .unwrap()
        .get::<i32, _>(0);
    assert_ne!(Some(1251), default_code_page);

    // A plain batch, not `execute`: a `#` temp table created inside the
    // sp_executesql call `execute` sends is dropped when that call returns.
    conn.simple_query(format!(
        "CREATE TABLE {} (id INT NOT NULL, \
         v VARCHAR(20) COLLATE Cyrillic_General_CI_AS NOT NULL, \
         c CHAR(6) COLLATE Cyrillic_General_CI_AS NOT NULL, \
         t TEXT COLLATE Cyrillic_General_CI_AS NOT NULL, \
         n NVARCHAR(20) COLLATE Cyrillic_General_CI_AS NOT NULL)",
        table
    ))
    .await?
    .into_results()
    .await?;

    let expected = "Привет";
    let mut req = conn.bulk_insert(table).await?;
    let mut row = TokenRow::new();
    row.push(1i32.into_sql());
    row.push(expected.into_sql());
    row.push(expected.into_sql());
    row.push(expected.into_sql());
    row.push(expected.into_sql());
    req.send(row).await?;
    assert_eq!(1, req.finalize().await?.total()?);

    let row = conn
        .query(
            &format!(
                "SELECT v, c, t, n, CONVERT(VARBINARY(20), v), CONVERT(VARBINARY(20), c), \
                 CONVERT(VARBINARY(20), CONVERT(VARCHAR(20), t)) FROM {}",
                table
            ),
            &[],
        )
        .await?
        .into_row()
        .await?
        .unwrap();

    let cp1251: &[u8] = &[0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
    assert_eq!(Some(expected), row.get::<&str, _>(0));
    assert_eq!(Some(expected), row.get::<&str, _>(1));
    assert_eq!(Some(expected), row.get::<&str, _>(2));
    assert_eq!(Some(expected), row.get::<&str, _>(3));
    assert_eq!(Some(cp1251), row.get::<&[u8], _>(4));
    assert_eq!(Some(cp1251), row.get::<&[u8], _>(5));
    assert_eq!(Some(cp1251), row.get::<&[u8], _>(6));

    Ok(())
}

#[test_on_runtimes]
async fn bulk_text_keeps_a_non_default_column_collation<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    // A table in the current database, named with its schema.
    let table = format!("dbo.bulk_collate_{}", random_table().await);

    let result = bulk_cyrillic_roundtrip(&mut conn, &table).await;
    drop_table(&mut conn, &format!("N'{table}'"), &table).await?;

    result
}

/// Drops `table`, whose `OBJECT_ID` name is `object_name`, in a plain batch.
async fn drop_table<S>(conn: &mut mssql::Client<S>, object_name: &str, table: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    conn.simple_query(format!(
        "IF OBJECT_ID({object_name}) IS NOT NULL DROP TABLE {table}"
    ))
    .await?
    .into_results()
    .await?;

    Ok(())
}

#[test_on_runtimes]
async fn bulk_text_keeps_a_non_default_column_collation_in_a_temp_table<S>(
    mut conn: mssql::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("#{}", random_table().await);

    let result = bulk_cyrillic_roundtrip(&mut conn, &table).await;
    drop_table(&mut conn, &format!("N'tempdb..{table}'"), &table).await?;

    result
}

// Bulk-inserting into TEXT/NTEXT columns used to fail with "premature end of
// message" (error 4804) because the bulk COLMETADATA omitted the table name
// those types carry (fork issue #352). Ported from upstream Tiberius
// (6eed8df, d10c7bb, 931223f).
#[test_on_runtimes]
async fn bulk_insert_text_and_ntext_round_trip<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (id INT NOT NULL, t TEXT NULL, nt NTEXT NULL, img IMAGE NULL)",
        table
    ))
    .await?;

    let long = "x".repeat(20_000);
    let mut req = conn.bulk_insert(&table).await?;
    for (id, t, nt, img) in [
        (
            1,
            Some("hello"),
            Some("héllo wörld ✓"),
            Some(&[1u8, 2, 3][..]),
        ),
        (2, None, None, None),
        (
            3,
            Some(long.as_str()),
            Some(long.as_str()),
            Some(long.as_bytes()),
        ),
    ] {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(id)));
        row.push(t.into_sql());
        row.push(nt.into_sql());
        row.push(img.into_sql());
        req.send(row).await?;
    }
    assert_eq!(req.finalize().await?.total()?, 3);

    let rows = conn
        .query(
            format!(
                "SELECT CAST(t AS VARCHAR(MAX)), CAST(nt AS NVARCHAR(MAX)), \
                 CAST(img AS VARBINARY(MAX)) FROM {} ORDER BY id",
                table
            ),
            &[],
        )
        .await?
        .into_first_result()
        .await?;

    assert_eq!(Some("hello"), rows[0].get::<&str, _>(0));
    assert_eq!(Some("héllo wörld ✓"), rows[0].get::<&str, _>(1));
    assert_eq!(None, rows[1].get::<&str, _>(0));
    assert_eq!(None, rows[1].get::<&str, _>(1));
    assert_eq!(Some(long.as_str()), rows[2].get::<&str, _>(0));
    assert_eq!(Some(long.as_str()), rows[2].get::<&str, _>(1));
    assert_eq!(Some(&[1u8, 2, 3][..]), rows[0].get::<&[u8], _>(2));
    assert_eq!(None, rows[1].get::<&[u8], _>(2));
    assert_eq!(Some(long.as_bytes()), rows[2].get::<&[u8], _>(2));

    Ok(())
}

// NOT NULL money/smallmoney columns (FixedLen on the wire) couldn't be
// bulk-inserted at all; Numeric values had no money encoding; and NaN or
// out-of-range f64 values were silently saturated. Ported from upstream
// Tiberius (302cfe4, e0a65d8, 1d91e18, 8510d35).
#[test_on_runtimes]
async fn bulk_insert_money_not_null_numeric_and_rejects_nan<S>(
    mut conn: mssql::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    use mssql::numeric::Numeric;

    let table = format!("##{}", random_table().await);

    conn.simple_query(format!(
        "CREATE TABLE {} (id INT NOT NULL, m MONEY NOT NULL, sm SMALLMONEY NOT NULL, mn MONEY NULL)",
        table
    ))
    .await?;

    let mut req = conn.bulk_insert(&table).await?;

    let mut nan = TokenRow::new();
    nan.push(ColumnData::I32(Some(0)));
    nan.push(ColumnData::F64(Some(f64::NAN)));
    nan.push(ColumnData::F64(Some(0.0)));
    nan.push(ColumnData::F64(None));
    assert!(req.send(nan).await.is_err());

    let mut row = TokenRow::new();
    row.push(ColumnData::I32(Some(1)));
    row.push(ColumnData::F64(Some(-12.3456)));
    row.push(ColumnData::F64(Some(214748.3647)));
    // Money's maximum, exact only through the Numeric path.
    row.push(ColumnData::Numeric(Some(Numeric::new_with_scale(
        9_223_372_036_854_775_807,
        4,
    ))));
    req.send(row).await?;

    let mut row = TokenRow::new();
    row.push(ColumnData::I32(Some(2)));
    row.push(ColumnData::Numeric(Some(Numeric::new_with_scale(
        123_456_789,
        5,
    ))));
    row.push(ColumnData::Numeric(Some(Numeric::new_with_scale(-1, 0))));
    row.push(ColumnData::F64(None));
    req.send(row).await?;

    assert_eq!(req.finalize().await?.total()?, 2);

    let rows = conn
        .query(
            format!(
                "SELECT CAST(m AS VARCHAR(40)), CAST(sm AS VARCHAR(40)), CAST(mn AS VARCHAR(40)) \
                 FROM {} ORDER BY id",
                table
            ),
            &[],
        )
        .await?
        .into_first_result()
        .await?;

    assert_eq!(Some("-12.35"), rows[0].get::<&str, _>(0)); // VARCHAR shows 2 places
    assert_eq!(Some("214748.36"), rows[0].get::<&str, _>(1));
    assert_eq!(Some("922337203685477.58"), rows[0].get::<&str, _>(2));
    assert_eq!(Some("1234.57"), rows[1].get::<&str, _>(0));
    assert_eq!(Some("-1.00"), rows[1].get::<&str, _>(1));
    assert_eq!(None, rows[1].get::<&str, _>(2));

    let exact = conn
        .query(
            format!(
                "SELECT CAST(mn AS DECIMAL(19,4)), CAST(m AS DECIMAL(19,4)) FROM {} ORDER BY id",
                table
            ),
            &[],
        )
        .await?
        .into_first_result()
        .await?;
    let max: Numeric = exact[0].get(0).unwrap();
    assert_eq!(max.value(), 9_223_372_036_854_775_807);
    let rounded: Numeric = exact[1].get(1).unwrap();
    assert_eq!(rounded.value(), 12_345_679); // 1234.56789 -> 1234.5679

    Ok(())
}

// CP437/CP850 collations used to fail with "unsupported encoding" in both
// directions. Ported from upstream Tiberius (75d7afc, 30f7b36).
/// Bulk-insert every byte 0x80..=0xFF of the code page of `collation`, as
/// text decoded by the client, into char, varchar, varchar(max) and text
/// columns of the # temp table `table` and check the stored bytes.
async fn bulk_every_high_byte_roundtrip<S>(
    conn: &mut mssql::Client<S>,
    table: &str,
    collation: &str,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let bytes: Vec<u8> = (0x80..=0xFF).collect();
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();

    conn.simple_query(format!(
        "CREATE TABLE {table} (c CHAR(128) COLLATE {collation} NOT NULL, \
         v VARCHAR(128) COLLATE {collation} NOT NULL, \
         m VARCHAR(MAX) COLLATE {collation} NOT NULL, \
         t TEXT COLLATE {collation} NOT NULL)"
    ))
    .await?
    .into_results()
    .await?;

    // The text of the 128 bytes: a varchar value of the collation (a binary
    // value converts to it byte for byte), decoded by the client, and the
    // server's own decoding of it for comparison.
    let row = conn
        .simple_query(format!(
            "DECLARE @t TABLE (v VARCHAR(128) COLLATE {collation}); \
             INSERT INTO @t VALUES (0x{hex}); \
             SELECT v, CONVERT(NVARCHAR(128), v), CONVERT(VARBINARY(128), v) FROM @t"
        ))
        .await?
        .into_row()
        .await?
        .unwrap();
    assert_eq!(
        Some(bytes.as_slice()),
        row.get::<&[u8], _>(2),
        "{collation}"
    );
    let text = row.get::<&str, _>(0).unwrap().to_owned();
    assert_eq!(Some(text.as_str()), row.get::<&str, _>(1), "{collation}");
    assert_eq!(128, text.chars().count(), "{collation}");

    let mut req = conn.bulk_insert(table).await?;
    let mut row = TokenRow::new();
    for _ in 0..4 {
        row.push(text.clone().into_sql());
    }
    req.send(row).await?;
    assert_eq!(1, req.finalize().await?.total()?);

    let row = conn
        .simple_query(format!(
            "SELECT CONVERT(VARBINARY(MAX), c), CONVERT(VARBINARY(MAX), v), \
             CONVERT(VARBINARY(MAX), m), \
             CONVERT(VARBINARY(MAX), CONVERT(VARCHAR(MAX), t)) FROM {table}"
        ))
        .await?
        .into_row()
        .await?
        .unwrap();

    for (i, column) in ["c", "v", "m", "t"].into_iter().enumerate() {
        assert_eq!(
            Some(bytes.as_slice()),
            row.get::<&[u8], _>(i),
            "{collation} column {column}"
        );
    }

    Ok(())
}

#[test_on_runtimes]
async fn bulk_legacy_code_pages_store_every_high_byte<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    // Detecting a missing `COLLATE` in `INSERT BULK` needs a database default
    // code page other than the columns'.
    let default_code_page = conn
        .query(
            "SELECT CONVERT(INT, COLLATIONPROPERTY(CONVERT(NVARCHAR(128), \
             DATABASEPROPERTYEX(DB_NAME(), 'Collation')), 'CodePage'))",
            &[],
        )
        .await?
        .into_row()
        .await?
        .unwrap()
        .get::<i32, _>(0);

    for (collation, code_page) in [
        ("SQL_Latin1_General_CP437_BIN", 437),
        ("SQL_1xCompat_CP850_CI_AS", 850),
    ] {
        assert_ne!(Some(code_page), default_code_page);

        let table = format!("#{}", random_table().await);
        let result = bulk_every_high_byte_roundtrip(&mut conn, &table, collation).await;
        drop_table(&mut conn, &format!("N'tempdb..{table}'"), &table).await?;
        result?;
    }

    Ok(())
}

// Bulk-inserting into a sql_variant column (ported from upstream Tiberius
// dd19eeb): each value goes in as a variant of its own base type.
#[test_on_runtimes]
async fn bulk_insert_into_sql_variant_column<S>(mut conn: mssql::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);
    conn.simple_query(format!(
        "CREATE TABLE {} (id INT NOT NULL, v sql_variant NULL)",
        table
    ))
    .await?;

    let values = [
        ColumnData::I32(Some(42)),
        ColumnData::String(Some("héllo".into())),
        ColumnData::F64(Some(1.5)),
        ColumnData::Binary(Some((&[0xDE, 0xAD][..]).into())),
        ColumnData::I64(None),
    ];
    let mut req = conn.bulk_insert(&table).await?;
    for (i, v) in values.iter().enumerate() {
        let mut row = TokenRow::new();
        row.push(ColumnData::I32(Some(i as i32)));
        row.push(v.clone());
        req.send(row).await?;
    }
    assert_eq!(req.finalize().await?.total()?, 5);

    let rows = conn
        .query(
            format!(
                "SELECT v, CAST(SQL_VARIANT_PROPERTY(v, 'BaseType') AS NVARCHAR(30)) \
                 FROM {} ORDER BY id",
                table
            ),
            &[],
        )
        .await?
        .into_first_result()
        .await?;

    assert_eq!(Some(42i32), rows[0].get(0));
    assert_eq!(Some("int"), rows[0].get::<&str, _>(1));
    assert_eq!(Some("héllo"), rows[1].get::<&str, _>(0));
    assert_eq!(Some("nvarchar"), rows[1].get::<&str, _>(1));
    assert_eq!(Some(1.5f64), rows[2].get(0));
    assert_eq!(Some(&[0xDE, 0xAD][..]), rows[3].get::<&[u8], _>(0));
    assert_eq!(None, rows[4].get::<&str, _>(1));

    Ok(())
}
