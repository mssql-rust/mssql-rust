//! Serializing a query result row to JSON and back with the optional `serde`
//! feature ([`Column`], [`ColumnData`], [`TokenRow`], and friends all derive
//! `Serialize`/`Deserialize` when it's enabled). This is the shape you'd use
//! to ship a row across a network boundary — e.g. a web API that forwards
//! query results as JSON instead of the TDS wire format.
//!
//! This example builds its row by hand instead of connecting to a live SQL
//! Server, so there's nothing to configure — just run it. Requires the
//! `serde` feature:
//!
//! ```sh
//! cargo run --example serde_json --features serde
//! ```
use std::borrow::Cow;
use std::sync::Arc;

use mssql::{Column, ColumnData, ColumnType, TokenRow};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Column metadata, as you'd get back from `QueryStream::columns()`.
    let columns = Arc::new(vec![
        Column::new("id".to_string(), ColumnType::Int4),
        Column::new("name".to_string(), ColumnType::NVarchar),
    ]);

    // A single row's data, as you'd get back from `QueryStream::into_row()`.
    let mut row: TokenRow<'static> = TokenRow::new();
    row.push(ColumnData::I32(Some(7)));
    row.push(ColumnData::String(Some(Cow::Owned("ada".to_string()))));

    // Serialize both to JSON, e.g. to send over the wire to a client.
    let columns_json = serde_json::to_string_pretty(&columns)?;
    let row_json = serde_json::to_string_pretty(&row)?;

    println!("columns:\n{columns_json}\n");
    println!("row:\n{row_json}\n");

    // Deserialize them back and confirm the round trip preserved the data.
    let columns_back: Arc<Vec<Column>> = serde_json::from_str(&columns_json)?;
    let row_back: TokenRow<'static> = serde_json::from_str(&row_json)?;

    assert_eq!(columns_back.len(), 2);
    assert_eq!(columns_back[0].name(), "id");
    assert_eq!(columns_back[1].column_type(), ColumnType::NVarchar);
    assert_eq!(row_back.get(0), Some(&ColumnData::I32(Some(7))));
    match row_back.get(1) {
        Some(ColumnData::String(Some(s))) => assert_eq!(s.as_ref(), "ada"),
        other => panic!("unexpected: {other:?}"),
    }

    println!("round trip OK");

    Ok(())
}
