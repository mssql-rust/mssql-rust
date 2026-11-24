use crate::{
    error::Error,
    tds::codec::{ColumnData, FixedLenType, TokenRow, TypeInfo, VarLenType},
    FromSql,
};
use std::{fmt::Display, sync::Arc};

/// A column of data from a query.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Column {
    pub(crate) name: String,
    pub(crate) column_type: ColumnType,
}

impl Column {
    /// Construct a new Column.
    pub fn new(name: String, column_type: ColumnType) -> Self {
        Self { name, column_type }
    }

    /// The name of the column.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The type of the column.
    pub fn column_type(&self) -> ColumnType {
        self.column_type
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The type of the column.
pub enum ColumnType {
    /// The column doesn't have a specified type.
    Null,
    /// A bit or boolean value.
    Bit,
    /// An 8-bit integer value.
    Int1,
    /// A 16-bit integer value.
    Int2,
    /// A 32-bit integer value.
    Int4,
    /// A 64-bit integer value.
    Int8,
    /// A 32-bit datetime value.
    Datetime4,
    /// A 32-bit floating point value.
    Float4,
    /// A 64-bit floating point value.
    Float8,
    /// Money value.
    Money,
    /// A TDS 7.2 datetime value.
    Datetime,
    /// A 32-bit money value.
    Money4,
    /// A unique identifier, UUID.
    Guid,
    /// N-bit integer value (variable).
    Intn,
    /// A bit value in a variable-length type.
    Bitn,
    /// A decimal value (same as `Numericn`).
    Decimaln,
    /// A numeric value (same as `Decimaln`).
    Numericn,
    /// A n-bit floating point value.
    Floatn,
    /// A n-bit datetime value (TDS 7.2).
    Datetimen,
    /// A n-bit date value (TDS 7.3).
    Daten,
    /// A n-bit time value (TDS 7.3).
    Timen,
    /// A n-bit datetime2 value (TDS 7.3).
    Datetime2,
    /// A n-bit datetime value with an offset (TDS 7.3).
    DatetimeOffsetn,
    /// A variable binary value.
    BigVarBin,
    /// A large variable string value.
    BigVarChar,
    /// A binary value.
    BigBinary,
    /// A string value.
    BigChar,
    /// A variable string value with UTF-16 encoding.
    NVarchar,
    /// A string value with UTF-16 encoding.
    NChar,
    /// A XML value.
    Xml,
    /// User-defined type.
    Udt,
    /// A text value (deprecated).
    Text,
    /// A image value (deprecated).
    Image,
    /// A text value with UTF-16 encoding (deprecated).
    NText,
    /// An SQL variant type.
    SSVariant,
}

impl From<&TypeInfo> for ColumnType {
    fn from(ti: &TypeInfo) -> Self {
        match ti {
            TypeInfo::FixedLen(flt) => match flt {
                FixedLenType::Int1 => Self::Int1,
                FixedLenType::Bit => Self::Bit,
                FixedLenType::Int2 => Self::Int2,
                FixedLenType::Int4 => Self::Int4,
                FixedLenType::Datetime4 => Self::Datetime4,
                FixedLenType::Float4 => Self::Float4,
                FixedLenType::Money => Self::Money,
                FixedLenType::Datetime => Self::Datetime,
                FixedLenType::Float8 => Self::Float8,
                FixedLenType::Money4 => Self::Money4,
                FixedLenType::Int8 => Self::Int8,
                FixedLenType::Null => Self::Null,
            },
            TypeInfo::VarLenSized(cx) => match cx.r#type() {
                VarLenType::Guid => Self::Guid,
                VarLenType::Intn => match cx.len() {
                    1 => Self::Int1,
                    2 => Self::Int2,
                    4 => Self::Int4,
                    8 => Self::Int8,
                    _ => Self::Intn,
                },
                VarLenType::Bitn => Self::Bitn,
                VarLenType::Decimaln => Self::Decimaln,
                VarLenType::Numericn => Self::Numericn,
                VarLenType::Floatn => match cx.len() {
                    4 => Self::Float4,
                    8 => Self::Float8,
                    _ => Self::Floatn,
                },
                VarLenType::Money => Self::Money,
                VarLenType::Datetimen => Self::Datetimen,
                #[cfg(feature = "tds73")]
                VarLenType::Daten => Self::Daten,
                #[cfg(feature = "tds73")]
                VarLenType::Timen => Self::Timen,
                #[cfg(feature = "tds73")]
                VarLenType::Datetime2 => Self::Datetime2,
                #[cfg(feature = "tds73")]
                VarLenType::DatetimeOffsetn => Self::DatetimeOffsetn,
                VarLenType::BigVarBin => Self::BigVarBin,
                VarLenType::BigVarChar => Self::BigVarChar,
                VarLenType::BigBinary => Self::BigBinary,
                VarLenType::BigChar => Self::BigChar,
                VarLenType::NVarchar => Self::NVarchar,
                VarLenType::NChar => Self::NChar,
                VarLenType::Xml => Self::Xml,
                VarLenType::Udt => Self::Udt,
                VarLenType::Text => Self::Text,
                VarLenType::Image => Self::Image,
                VarLenType::NText => Self::NText,
                VarLenType::SSVariant => Self::SSVariant,
            },
            TypeInfo::VarLenSizedPrecision { ty, .. } => match ty {
                VarLenType::Guid => Self::Guid,
                VarLenType::Intn => Self::Intn,
                VarLenType::Bitn => Self::Bitn,
                VarLenType::Decimaln => Self::Decimaln,
                VarLenType::Numericn => Self::Numericn,
                VarLenType::Floatn => Self::Floatn,
                VarLenType::Money => Self::Money,
                VarLenType::Datetimen => Self::Datetimen,
                #[cfg(feature = "tds73")]
                VarLenType::Daten => Self::Daten,
                #[cfg(feature = "tds73")]
                VarLenType::Timen => Self::Timen,
                #[cfg(feature = "tds73")]
                VarLenType::Datetime2 => Self::Datetime2,
                #[cfg(feature = "tds73")]
                VarLenType::DatetimeOffsetn => Self::DatetimeOffsetn,
                VarLenType::BigVarBin => Self::BigVarBin,
                VarLenType::BigVarChar => Self::BigVarChar,
                VarLenType::BigBinary => Self::BigBinary,
                VarLenType::BigChar => Self::BigChar,
                VarLenType::NVarchar => Self::NVarchar,
                VarLenType::NChar => Self::NChar,
                VarLenType::Xml => Self::Xml,
                VarLenType::Udt => Self::Udt,
                VarLenType::Text => Self::Text,
                VarLenType::Image => Self::Image,
                VarLenType::NText => Self::NText,
                VarLenType::SSVariant => Self::SSVariant,
            },
            TypeInfo::Xml { .. } => Self::Xml,
        }
    }
}

/// A row of data from a query.
///
/// Data can be accessed either by copying through [`get`] or [`try_get`]
/// methods, or moving by value using the [`IntoIterator`] implementation.
///
/// ```
/// # use mssql::{Config, FromSqlOwned};
/// # use tokio_util::compat::TokioAsyncWriteCompatExt;
/// # use std::env;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
/// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
/// # );
/// # let config = Config::from_ado_string(&c_str)?;
/// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
/// # tcp.set_nodelay(true)?;
/// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
/// // by-reference
/// let row = client
///     .query("SELECT @P1 AS col1", &[&"test"])
///     .await?
///     .into_row()
///     .await?
///     .unwrap();
///
/// assert_eq!(Some("test"), row.get("col1"));
///
/// // ...or by-value
/// let row = client
///     .query("SELECT @P1 AS col1", &[&"test"])
///     .await?
///     .into_row()
///     .await?
///     .unwrap();
///
/// for val in row.into_iter() {
///     assert_eq!(
///         Some(String::from("test")),
///         String::from_sql_owned(val)?
///     )
/// }
/// # Ok(())
/// # }
/// ```
///
/// [`get`]: #method.get
/// [`try_get`]: #method.try_get
/// [`IntoIterator`]: #impl-IntoIterator
// `PartialEq`, not `Eq`: `ColumnData` (via `TokenRow`) can hold `f32`/`f64`,
// and an `Eq` impl would be dishonest about that - NaN != NaN breaks `Eq`'s
// reflexivity contract even though nothing in the type system stops you
// from deriving it anyway (#402).
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Row {
    pub(crate) columns: Arc<Vec<Column>>,
    pub(crate) data: TokenRow<'static>,
    pub(crate) result_index: usize,
}

/// A type that can be used to index into a [`Row`], resolving to a column's
/// position. Implemented for `usize` (a zero-based column index) and `&str`
/// (a column name); implement it for your own type to write a generic
/// wrapper around [`Row::get`]/[`Row::try_get`].
pub trait QueryIdx
where
    Self: Display,
{
    /// Resolves `self` to a column's zero-based position in `row`, or `None`
    /// if no such column exists.
    fn idx(&self, row: &Row) -> Option<usize>;
}

/// Builds a [`Row`] outside of a query, mainly to unit-test code that takes
/// a `Row` or `&[Row]` without a live connection. Obtained from
/// [`Row::builder`]. Ported from upstream Tiberius (tiberius-rs/tiberius
/// 30b059c, upstream issue #383), alongside this fork's existing
/// [`Row::new`].
///
/// # Example
///
/// ```
/// use mssql::{ColumnData, ColumnType, Row};
///
/// let row = Row::builder()
///     .column("id", ColumnType::Int4, ColumnData::I32(Some(1)))
///     .column("name", ColumnType::NVarchar, ColumnData::String(Some("Alice".into())))
///     .column("age", ColumnType::Int4, ColumnData::I32(None))
///     .build();
///
/// assert_eq!(Some(1), row.get::<i32, _>("id"));
/// assert_eq!(Some("Alice"), row.get::<&str, _>(1));
/// assert_eq!(None, row.get::<i32, _>("age"));
/// ```
#[derive(Debug, Default)]
pub struct RowBuilder {
    columns: Vec<Column>,
    data: TokenRow<'static>,
    result_index: usize,
}

impl RowBuilder {
    /// Appends a column and its value. `column_type` is what
    /// [`Column::column_type`] reports; it isn't checked against `value`,
    /// since the accessors dispatch on the [`ColumnData`] variant.
    pub fn column(
        mut self,
        name: impl Into<String>,
        column_type: ColumnType,
        value: ColumnData<'static>,
    ) -> Self {
        self.columns.push(Column::new(name.into(), column_type));
        self.data.push(value);
        self
    }

    /// Sets what [`Row::result_index`] reports (default 0).
    pub fn result_index(mut self, result_index: usize) -> Self {
        self.result_index = result_index;
        self
    }

    /// Finishes the row.
    pub fn build(self) -> Row {
        Row {
            columns: Arc::new(self.columns),
            data: self.data,
            result_index: self.result_index,
        }
    }
}

impl QueryIdx for usize {
    fn idx(&self, row: &Row) -> Option<usize> {
        if *self < row.len() {
            Some(*self)
        } else {
            None
        }
    }
}

impl QueryIdx for &str {
    fn idx(&self, row: &Row) -> Option<usize> {
        // A `FromRow`-style derive macro built on `stringify!(field)` produces
        // `r#type` for a field named after a Rust keyword; strip that prefix
        // so it matches a column literally named `type` (which is not itself
        // a valid raw-identifier-prefixed SQL name, so this can't shadow a
        // real column).
        //
        // An exact match wins first, so a column whose name really starts
        // with `r#` (legal when bracket-quoted) stays addressable (upstream
        // Tiberius bf25ac6).
        row.columns
            .iter()
            .position(|c| c.name() == *self)
            .or_else(|| {
                let name = self.strip_prefix("r#")?;
                row.columns.iter().position(|c| c.name() == name)
            })
    }
}

impl Row {
    /// Construct a `Row` from its columns and data, for example to build
    /// fixture values in a test without a live connection.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::{Column, ColumnData, ColumnType, Row, TokenRow};
    /// let columns = vec![Column::new("id".into(), ColumnType::Int4)];
    ///
    /// let mut data = TokenRow::new();
    /// data.push(ColumnData::I32(Some(1)));
    ///
    /// let row = Row::new(columns, data);
    /// assert_eq!(Some(1i32), row.get("id"));
    /// ```
    pub fn new(columns: Vec<Column>, data: TokenRow<'static>) -> Self {
        Self {
            columns: Arc::new(columns),
            data,
            result_index: 0,
        }
    }

    /// A [`RowBuilder`] that adds each column together with its value, so
    /// the two lists can't drift out of sync the way separate `columns` and
    /// `TokenRow` arguments to [`Row::new`] can.
    pub fn builder() -> RowBuilder {
        RowBuilder::default()
    }

    /// Columns defining the row data. Columns listed here are in the same order
    /// as the resulting data.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let row = client
    ///     .query("SELECT 1 AS foo, 2 AS bar", &[])
    ///     .await?
    ///     .into_row()
    ///     .await?
    ///     .unwrap();
    ///
    /// assert_eq!("foo", row.columns()[0].name());
    /// assert_eq!("bar", row.columns()[1].name());
    /// # Ok(())
    /// # }
    /// ```
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Return an iterator over row column-value pairs.
    pub fn cells(&self) -> impl Iterator<Item = (&Column, &ColumnData<'static>)> {
        self.columns().iter().zip(self.data.iter())
    }

    /// The result set number, starting from zero and increasing if the stream
    /// has results from more than one query.
    pub fn result_index(&self) -> usize {
        self.result_index
    }

    /// Returns the number of columns in the row.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let row = client
    ///     .query("SELECT 1, 2", &[])
    ///     .await?
    ///     .into_row()
    ///     .await?
    ///     .unwrap();
    ///
    /// assert_eq!(2, row.len());
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Retrieve a column value for a given column index, which can either be
    /// the zero-indexed position or the name of the column.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let row = client
    ///     .query("SELECT @P1 AS col1", &[&1i32])
    ///     .await?
    ///     .into_row()
    ///     .await?
    ///     .unwrap();
    ///
    /// assert_eq!(Some(1i32), row.get(0));
    /// assert_eq!(Some(1i32), row.get("col1"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// - The requested type conversion (SQL->Rust) is not possible.
    /// - The given index is out of bounds (column does not exist).
    ///
    /// Use [`try_get`] for a non-panicking version of the function.
    ///
    /// [`try_get`]: #method.try_get
    #[track_caller]
    pub fn get<'a, R, I>(&'a self, idx: I) -> Option<R>
    where
        R: FromSql<'a>,
        I: QueryIdx,
    {
        self.try_get(idx).unwrap()
    }

    /// Retrieve a column's value for a given column index.
    #[track_caller]
    pub fn try_get<'a, R, I>(&'a self, idx: I) -> crate::Result<Option<R>>
    where
        R: FromSql<'a>,
        I: QueryIdx,
    {
        let data = self.get_column_data(idx)?;

        R::from_sql(data)
    }

    /// Retrieve the raw data for a column without first knowing its Rust type.
    ///
    /// This is the dynamic counterpart to [`get`](Self::get): callers can
    /// inspect the returned [`ColumnData`] variant and decide how to handle it.
    ///
    /// # Errors
    ///
    /// Returns an error if `idx` does not name a column, or if malformed row
    /// data contains fewer cells than its column metadata.
    #[track_caller]
    pub fn get_column_data<I>(&self, idx: I) -> crate::Result<&ColumnData<'static>>
    where
        I: QueryIdx,
    {
        let column_idx = idx.idx(self).ok_or_else(|| {
            Error::Conversion(format!("Could not find column with index {}", idx).into())
        })?;

        self.data.get(column_idx).ok_or_else(|| {
            Error::Protocol(format!("row has no data for column index {column_idx}").into())
        })
    }

    /// Retrieve a column value by name using an ASCII case-insensitive lookup.
    ///
    /// An exact match always wins. This method is opt-in so existing
    /// [`get`](Self::get) calls retain their case-sensitive behavior and so
    /// schemas containing names that differ only by case remain unambiguous.
    #[track_caller]
    pub fn get_case_insensitive<'a, R>(&'a self, name: &str) -> Option<R>
    where
        R: FromSql<'a>,
    {
        self.try_get_case_insensitive(name).unwrap()
    }

    /// Non-panicking version of [`get_case_insensitive`](Self::get_case_insensitive).
    #[track_caller]
    pub fn try_get_case_insensitive<'a, R>(&'a self, name: &str) -> crate::Result<Option<R>>
    where
        R: FromSql<'a>,
    {
        let column_idx = self
            .columns
            .iter()
            .position(|column| column.name() == name)
            .or_else(|| {
                self.columns
                    .iter()
                    .position(|column| column.name().eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| {
                Error::Conversion(format!("Could not find column with name {name}").into())
            })?;

        let data = self.data.get(column_idx).ok_or_else(|| {
            Error::Protocol(format!("row has no data for column index {column_idx}").into())
        })?;

        R::from_sql(data)
    }
}

impl IntoIterator for Row {
    type Item = ColumnData<'static>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

/// Lets a `Row` read from a query be sent straight back into a
/// [`BulkLoadRequest`](crate::BulkLoadRequest) or another statement's
/// parameters, without manually copying each cell.
impl From<Row> for TokenRow<'static> {
    fn from(row: Row) -> Self {
        row.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_round_trips_through_public_accessors() {
        let row = Row::builder()
            .column("id", ColumnType::Int4, ColumnData::I32(Some(1)))
            .column(
                "name",
                ColumnType::NVarchar,
                ColumnData::String(Some("Alice".into())),
            )
            .column("flag", ColumnType::Bit, ColumnData::Bit(Some(true)))
            .column("age", ColumnType::Int4, ColumnData::I32(None))
            .result_index(3)
            .build();

        assert_eq!(4, row.len());
        assert_eq!(3, row.result_index());
        assert_eq!(ColumnType::NVarchar, row.columns()[1].column_type());
        assert_eq!(Some("Alice"), row.get::<&str, _>("name"));
        assert_eq!(Some(true), row.get::<bool, _>(2));
        assert_eq!(None, row.get::<i32, _>("age"));
        assert!(row.try_get::<i32, _>("nope").is_err());

        let mut data = TokenRow::new();
        data.push(ColumnData::I32(Some(7)));
        assert_eq!(
            Row::builder()
                .column("x", ColumnType::Int4, ColumnData::I32(Some(7)))
                .build(),
            Row::new(vec![Column::new("x".into(), ColumnType::Int4)], data)
        );
    }

    #[test]
    fn exact_column_name_wins_over_raw_identifier_stripping() {
        let mut data = TokenRow::new();
        data.push(ColumnData::I32(Some(1)));
        data.push(ColumnData::I32(Some(2)));
        let row = Row::new(
            vec![
                Column::new("type".into(), ColumnType::Int4),
                Column::new("r#type".into(), ColumnType::Int4),
            ],
            data,
        );
        assert_eq!(Some(2i32), row.get("r#type"));
        assert_eq!(Some(1i32), row.get("type"));

        // With no literal `r#type` column, `r#type` still finds `type`.
        let mut data = TokenRow::new();
        data.push(ColumnData::I32(Some(1)));
        let row = Row::new(vec![Column::new("type".into(), ColumnType::Int4)], data);
        assert_eq!(Some(1i32), row.get("r#type"));
    }

    // Regression test for #402: `Row` (and `Column`/`TokenRow`, which it's
    // built from) had no `PartialEq`, so test code couldn't
    // `assert_eq!` a `Row` built with `Row::new` against one read back
    // from a query without manually comparing cell-by-cell. Deliberately
    // `PartialEq` only, not `Eq` - see the derive site above.
    #[test]
    fn rows_with_equal_columns_and_data_are_equal() {
        let columns = vec![Column::new("id".into(), ColumnType::Int4)];

        let mut data = TokenRow::new();
        data.push(ColumnData::I32(Some(1)));

        let a = Row::new(columns.clone(), data.clone());
        let b = Row::new(columns, data);

        assert_eq!(a, b);
    }

    #[test]
    fn rows_with_different_data_are_not_equal() {
        let columns = vec![Column::new("id".into(), ColumnType::Int4)];

        let mut data_a = TokenRow::new();
        data_a.push(ColumnData::I32(Some(1)));

        let mut data_b = TokenRow::new();
        data_b.push(ColumnData::I32(Some(2)));

        let a = Row::new(columns.clone(), data_a);
        let b = Row::new(columns, data_b);

        assert_ne!(a, b);
    }

    #[test]
    fn raw_column_data_is_available_by_name_and_index() {
        let row = Row::builder()
            .column("value", ColumnType::Int4, ColumnData::I32(Some(42)))
            .build();

        assert_eq!(
            &ColumnData::I32(Some(42)),
            row.get_column_data("value").unwrap()
        );
        assert_eq!(
            row.get_column_data(0).unwrap(),
            row.get_column_data("value").unwrap()
        );
        assert!(row.get_column_data("missing").is_err());
        assert!(row.get_column_data(1).is_err());
    }

    #[test]
    fn raw_column_data_reports_malformed_rows() {
        let row = Row::new(
            vec![Column::new("missing".into(), ColumnType::Int4)],
            TokenRow::new(),
        );
        let error = row.get_column_data("missing").unwrap_err();
        assert!(matches!(error, Error::Protocol(_)));
    }

    #[test]
    fn case_insensitive_lookup_is_opt_in_and_exact_match_wins() {
        let row = Row::builder()
            .column("CHANGED_BY", ColumnType::Int4, ColumnData::I32(Some(1)))
            .column("changed_by", ColumnType::Int4, ColumnData::I32(Some(2)))
            .build();

        assert!(row.try_get::<i32, _>("Changed_By").is_err());
        assert_eq!(Some(1), row.get_case_insensitive::<i32>("CHANGED_BY"));
        assert_eq!(Some(2), row.get_case_insensitive::<i32>("changed_by"));
        assert_eq!(Some(1), row.get_case_insensitive::<i32>("Changed_By"));
        assert!(row.try_get_case_insensitive::<i32>("missing").is_err());
    }

    #[test]
    fn every_fixed_len_type_maps_to_a_public_column_type() {
        for (wire, public) in [
            (FixedLenType::Int1, ColumnType::Int1),
            (FixedLenType::Bit, ColumnType::Bit),
            (FixedLenType::Int2, ColumnType::Int2),
            (FixedLenType::Int4, ColumnType::Int4),
            (FixedLenType::Datetime4, ColumnType::Datetime4),
            (FixedLenType::Float4, ColumnType::Float4),
            (FixedLenType::Money, ColumnType::Money),
            (FixedLenType::Datetime, ColumnType::Datetime),
            (FixedLenType::Float8, ColumnType::Float8),
            (FixedLenType::Money4, ColumnType::Money4),
            (FixedLenType::Int8, ColumnType::Int8),
            (FixedLenType::Null, ColumnType::Null),
        ] {
            assert_eq!(public, ColumnType::from(&TypeInfo::FixedLen(wire)));
        }
    }

    #[test]
    fn every_var_len_type_maps_to_a_public_column_type() {
        use crate::VarLenContext;

        for (wire, public) in [
            (VarLenType::Guid, ColumnType::Guid),
            (VarLenType::Bitn, ColumnType::Bitn),
            (VarLenType::Decimaln, ColumnType::Decimaln),
            (VarLenType::Numericn, ColumnType::Numericn),
            (VarLenType::Money, ColumnType::Money),
            (VarLenType::Datetimen, ColumnType::Datetimen),
            (VarLenType::BigVarBin, ColumnType::BigVarBin),
            (VarLenType::BigVarChar, ColumnType::BigVarChar),
            (VarLenType::BigBinary, ColumnType::BigBinary),
            (VarLenType::BigChar, ColumnType::BigChar),
            (VarLenType::NVarchar, ColumnType::NVarchar),
            (VarLenType::NChar, ColumnType::NChar),
            (VarLenType::Xml, ColumnType::Xml),
            (VarLenType::Udt, ColumnType::Udt),
            (VarLenType::Text, ColumnType::Text),
            (VarLenType::Image, ColumnType::Image),
            (VarLenType::NText, ColumnType::NText),
            (VarLenType::SSVariant, ColumnType::SSVariant),
        ] {
            let sized = TypeInfo::VarLenSized(VarLenContext::new(wire, 17, None));
            assert_eq!(public, ColumnType::from(&sized), "{wire:?}");

            let precision = TypeInfo::VarLenSizedPrecision {
                ty: wire,
                size: 17,
                precision: 10,
                scale: 2,
            };
            assert_eq!(public, ColumnType::from(&precision), "{wire:?}");
        }

        for (len, public) in [
            (1, ColumnType::Int1),
            (2, ColumnType::Int2),
            (4, ColumnType::Int4),
            (8, ColumnType::Int8),
            (3, ColumnType::Intn),
        ] {
            let ty = TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Intn, len, None));
            assert_eq!(public, ColumnType::from(&ty));
        }
        for (len, public) in [
            (4, ColumnType::Float4),
            (8, ColumnType::Float8),
            (3, ColumnType::Floatn),
        ] {
            let ty = TypeInfo::VarLenSized(VarLenContext::new(VarLenType::Floatn, len, None));
            assert_eq!(public, ColumnType::from(&ty));
        }
    }

    #[cfg(feature = "tds73")]
    #[test]
    fn every_tds73_type_maps_in_both_var_len_forms() {
        use crate::VarLenContext;

        for (wire, public) in [
            (VarLenType::Daten, ColumnType::Daten),
            (VarLenType::Timen, ColumnType::Timen),
            (VarLenType::Datetime2, ColumnType::Datetime2),
            (VarLenType::DatetimeOffsetn, ColumnType::DatetimeOffsetn),
        ] {
            let sized = TypeInfo::VarLenSized(VarLenContext::new(wire, 8, None));
            assert_eq!(public, ColumnType::from(&sized));
            let precision = TypeInfo::VarLenSizedPrecision {
                ty: wire,
                size: 8,
                precision: 0,
                scale: 7,
            };
            assert_eq!(public, ColumnType::from(&precision));
        }
    }

    #[test]
    fn xml_type_info_maps_to_xml() {
        let ty = TypeInfo::Xml {
            schema: None,
            size: 0,
        };
        assert_eq!(ColumnType::Xml, ColumnType::from(&ty));
    }

    #[test]
    fn precision_typed_columns_map_every_variable_length_type() {
        let precision = |ty| TypeInfo::VarLenSizedPrecision {
            ty,
            size: 17,
            precision: 38,
            scale: 2,
        };
        for (ty, expected) in [
            (VarLenType::Guid, ColumnType::Guid),
            (VarLenType::Intn, ColumnType::Intn),
            (VarLenType::Bitn, ColumnType::Bitn),
            (VarLenType::Decimaln, ColumnType::Decimaln),
            (VarLenType::Numericn, ColumnType::Numericn),
            (VarLenType::Floatn, ColumnType::Floatn),
            (VarLenType::Money, ColumnType::Money),
            (VarLenType::Datetimen, ColumnType::Datetimen),
            (VarLenType::BigVarBin, ColumnType::BigVarBin),
            (VarLenType::BigVarChar, ColumnType::BigVarChar),
            (VarLenType::BigBinary, ColumnType::BigBinary),
            (VarLenType::BigChar, ColumnType::BigChar),
            (VarLenType::NVarchar, ColumnType::NVarchar),
            (VarLenType::NChar, ColumnType::NChar),
            (VarLenType::Xml, ColumnType::Xml),
            (VarLenType::Udt, ColumnType::Udt),
            (VarLenType::Text, ColumnType::Text),
            (VarLenType::Image, ColumnType::Image),
            (VarLenType::NText, ColumnType::NText),
            (VarLenType::SSVariant, ColumnType::SSVariant),
        ] {
            assert_eq!(ColumnType::from(&precision(ty)), expected, "{ty:?}");
        }
        assert_eq!(
            ColumnType::from(&TypeInfo::FixedLen(FixedLenType::Null)),
            ColumnType::Null
        );
    }

    #[test]
    fn a_column_without_data_is_a_protocol_error() {
        let row = Row::new(
            vec![Column::new("id".into(), ColumnType::Int4)],
            TokenRow::new(),
        );
        assert!(matches!(
            row.try_get::<i32, _>("id"),
            Err(Error::Protocol(_))
        ));
    }
}
