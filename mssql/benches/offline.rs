//! CPU-only benchmarks of public API hot paths; no SQL Server needed.
//! Run with `cargo bench --bench offline`.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use mssql::{numeric::Numeric, ColumnData, ColumnType, Config, FromSql, IntoSql, Row};
use std::hint::black_box;

const ADO: &str = "Server=tcp:db.example.com,1433;Database=orders;User Id=app;\
                   Password='p@ss;w=rd';Encrypt=true;TrustServerCertificate=false;\
                   Application Name=bench;ApplicationIntent=ReadOnly;\
                   MultiSubnetFailover=true;Workstation ID=host-01";

fn connection_string(c: &mut Criterion) {
    c.bench_function("config/from_ado_string", |b| {
        b.iter(|| Config::from_ado_string(black_box(ADO)).unwrap())
    });
    c.bench_function("config/from_jdbc_string", |b| {
        b.iter(|| {
            Config::from_jdbc_string(black_box(
                "jdbc:sqlserver://db.example.com:1433;databaseName=orders;user=app;\
                 password={p@ss;w=rd};encrypt=true;applicationName=bench",
            ))
            .unwrap()
        })
    });
}

fn wide_row(columns: usize) -> Row {
    let mut b = Row::builder();
    for i in 0..columns {
        b = b.column(
            format!("col_{i}"),
            ColumnType::Int4,
            ColumnData::I32(Some(i as i32)),
        );
    }
    b.build()
}

fn row_access(c: &mut Criterion) {
    let row = wide_row(32);
    c.bench_function("row/get_by_index_x32", |b| {
        b.iter(|| {
            for i in 0..32usize {
                black_box(row.get::<i32, _>(i));
            }
        })
    });
    c.bench_function("row/get_by_name_x32", |b| {
        let names: Vec<String> = (0..32).map(|i| format!("col_{i}")).collect();
        b.iter(|| {
            for n in &names {
                black_box(row.get::<i32, _>(n.as_str()));
            }
        })
    });
    c.bench_function("row/builder_32_columns", |b| {
        b.iter(|| wide_row(black_box(32)))
    });
}

fn conversions(c: &mut Criterion) {
    c.bench_function("numeric/display", |b| {
        let n = Numeric::new_with_scale(-123_456_789_012_345_678, 6);
        b.iter(|| black_box(n).to_string())
    });
    c.bench_function("string/into_sql_and_back", |b| {
        b.iter_batched(
            || "a moderately long nvarchar value, é ✓".to_string(),
            |s| {
                let data = s.into_sql();
                black_box(<&str as FromSql>::from_sql(&data).unwrap().map(str::len))
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, connection_string, row_access, conversions);
criterion_main!(benches);
