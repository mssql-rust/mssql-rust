//! End-to-end benchmarks against a live SQL Server. Opt-in: set
//! MSSQL_TEST_CONNECTION_STRING (e.g. after `./docker/test-server.sh up`),
//! otherwise every benchmark is skipped. Run with `cargo bench --bench live`.
//! Numbers depend on the server and the machine; compare runs on the same setup.
//!
//! Each benchmark uses `iter_custom`: one connection is reused across
//! iterations inside a single `block_on`, so only the client work and the
//! round trips are timed (not connecting, runtime startup, or row building).

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use mssql::{Client, ColumnData, Config, TokenRow};
use std::time::{Duration, Instant};
use tokio::{net::TcpStream, runtime::Runtime};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

type Conn = Client<Compat<TcpStream>>;

async fn connect(conn_str: &str) -> Conn {
    let config = Config::from_ado_string(conn_str).unwrap();
    let tcp = TcpStream::connect(config.get_addr()).await.unwrap();
    tcp.set_nodelay(true).unwrap();
    Client::connect(config, tcp.compat_write()).await.unwrap()
}

const STREAM_SQL: &str = "SELECT TOP 10000 a.object_id, a.name, \
     CAST(a.create_date AS DATETIME2), CAST(a.object_id AS DECIMAL(18,2)) \
     FROM sys.all_objects a CROSS JOIN sys.all_objects b";

fn bulk_rows(n: i32) -> Vec<TokenRow<'static>> {
    (0..n)
        .map(|i| {
            let mut row = TokenRow::new();
            row.push(ColumnData::I32(Some(i)));
            row.push(ColumnData::String(Some(format!("name {i}").into())));
            row.push(ColumnData::F64(Some(f64::from(i) * 0.5)));
            row
        })
        .collect()
}

fn live(c: &mut Criterion) {
    let Ok(conn_str) = std::env::var("MSSQL_TEST_CONNECTION_STRING") else {
        eprintln!("skipping live benchmarks: MSSQL_TEST_CONNECTION_STRING is not set");
        return;
    };
    let rt = Runtime::new().unwrap();
    let mut conn = rt.block_on(connect(&conn_str));

    c.bench_function("live/connect_and_login", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    drop(connect(&conn_str).await);
                }
                start.elapsed()
            })
        })
    });

    c.bench_function("live/select_1_round_trip", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    conn.simple_query("SELECT 1")
                        .await
                        .unwrap()
                        .into_row()
                        .await
                        .unwrap();
                }
                start.elapsed()
            })
        })
    });

    c.bench_function("live/parameterized_query", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    conn.query("SELECT @P1 + @P2, @P3", &[&1i32, &2i32, &"text"])
                        .await
                        .unwrap()
                        .into_row()
                        .await
                        .unwrap();
                }
                start.elapsed()
            })
        })
    });

    let mut group = c.benchmark_group("live/stream_rows");
    group.throughput(Throughput::Elements(10_000));
    group.sample_size(20);
    group.bench_function("10k_rows_4_columns", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let rows = conn
                        .simple_query(STREAM_SQL)
                        .await
                        .unwrap()
                        .into_first_result()
                        .await
                        .unwrap();
                    assert_eq!(rows.len(), 10_000);
                }
                start.elapsed()
            })
        })
    });
    group.finish();

    rt.block_on(async {
        conn.simple_query(
            "IF OBJECT_ID('tempdb..##mssql_bench_bulk') IS NULL \
             CREATE TABLE ##mssql_bench_bulk \
             (id INT NOT NULL, name NVARCHAR(50) NULL, amount FLOAT NULL)",
        )
        .await
        .unwrap()
        .into_results()
        .await
        .unwrap();
    });

    let mut group = c.benchmark_group("live/bulk_insert");
    group.throughput(Throughput::Elements(10_000));
    group.sample_size(10);
    group.bench_function("10k_rows_3_columns", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let rows = bulk_rows(10_000); // built outside the timer
                    let start = Instant::now();
                    let mut req = conn.bulk_insert("##mssql_bench_bulk").await.unwrap();
                    for row in rows {
                        req.send(row).await.unwrap();
                    }
                    req.finalize().await.unwrap();
                    total += start.elapsed();
                }
                total
            })
        })
    });
    group.finish();

    rt.block_on(async {
        let _ = conn
            .simple_query("DROP TABLE IF EXISTS ##mssql_bench_bulk")
            .await;
    });
}

criterion_group!(benches, live);
criterion_main!(benches);
