use crate::{
    tds::{codec::ColumnData, Numeric},
    xml::XmlData,
};
use std::borrow::Cow;
use uuid::Uuid;

/// A conversion trait to a TDS type.
///
/// A `ToSql` implementation for a Rust type is needed for using it as a
/// parameter in the [`Client::query`](crate::Client::query) or
/// [`Client::execute`](crate::Client::execute) methods. The following Rust
/// types are already implemented to match the given server types:
///
/// |Rust type|Server type|
/// |--------|--------|
/// |`u8`|`tinyint`|
/// |`i16`|`smallint`|
/// |`i32`|`int`|
/// |`i64`|`bigint`|
/// |`f32`|`float(24)`|
/// |`f64`|`float(53)`|
/// |`bool`|`bit`|
/// |`String`/`&str` (< 4000 characters)|`nvarchar(4000)`|
/// |`String`/`&str`|`nvarchar(max)`|
/// |`Vec<u8>`/`&[u8]` (< 8000 bytes)|`varbinary(8000)`|
/// |`Vec<u8>`/`&[u8]`|`varbinary(max)`|
/// |[`Uuid`]|`uniqueidentifier`|
/// |[`Numeric`]|`numeric`/`decimal`|
/// |`Decimal` (with feature flag `rust_decimal`, see [`crate::numeric`])|`numeric`/`decimal`|
/// |`BigDecimal` (with feature flag `bigdecimal`, see [`crate::numeric`])|`numeric`/`decimal`|
/// |[`XmlData`]|`xml`|
/// |`jiff::civil::Date` (with `jiff` feature, TDS 7.3 >)|`date`|
/// |`jiff::civil::Time` (with `jiff` feature, TDS 7.3 >)|`time`|
/// |`jiff::Zoned` (with `jiff` feature, TDS 7.3 >)|`datetimeoffset`|
/// |`jiff::Timestamp` (with `jiff` feature, TDS 7.3 >)|`datetime2` (UTC)|
/// |`jiff::civil::DateTime` (with `jiff` feature, TDS 7.3 >)|`datetime2`|
/// |`jiff::civil::DateTime` (with `jiff` feature, TDS 7.2)|`datetime`|
///
/// It is possible to use some of the types to write into columns that are not
/// of the same type. For example on systems following the TDS 7.3 standard (SQL
/// Server 2008 and later), the jiff type `civil::DateTime` can also be used to
/// write to `datetime`, `datetime2` and `smalldatetime` columns. All string
/// types can also be used with `ntext`, `text`, `varchar`, `nchar` and `char`
/// columns. All binary types can also be used with `binary` and `image`
/// columns.
///
/// See the [`time`](crate::time) module for more information about the date
/// and time structs.
pub trait ToSql: Send + Sync {
    /// Convert to a value understood by the SQL Server. Conversion
    /// by-reference.
    fn to_sql(&self) -> ColumnData<'_>;
}

// Lets a struct holding a `&dyn ToSql`/`Box<dyn ToSql>` field (e.g. a query
// builder) derive `Debug` (#404). Adding `Debug` as a supertrait on `ToSql`
// itself would be a breaking change for every external implementor, so this
// implements it just for the trait object instead, delegating to the
// `ColumnData` that `to_sql()` already produces (which derives `Debug`).
impl std::fmt::Debug for dyn ToSql + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.to_sql(), f)
    }
}

/// A by-value conversion trait to a TDS type.
pub trait IntoSql<'a>: Send + Sync {
    /// Convert to a value understood by the SQL Server. Conversion by-value.
    fn into_sql(self) -> ColumnData<'a>;
}

impl<'a> IntoSql<'a> for &'a str {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(Some(Cow::Borrowed(self)))
    }
}

impl<'a> IntoSql<'a> for Option<&'a str> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(self.map(Cow::Borrowed))
    }
}

impl<'a> IntoSql<'a> for &'a String {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(Some(Cow::Borrowed(self)))
    }
}

impl<'a> IntoSql<'a> for Option<&'a String> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(self.map(Cow::from))
    }
}

impl<'a> IntoSql<'a> for &'a [u8] {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(Some(Cow::Borrowed(self)))
    }
}

impl<'a> IntoSql<'a> for Option<&'a [u8]> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(self.map(Cow::Borrowed))
    }
}

impl<'a> IntoSql<'a> for &'a Vec<u8> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(Some(Cow::from(self)))
    }
}

impl<'a> IntoSql<'a> for Option<&'a Vec<u8>> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(self.map(Cow::from))
    }
}

impl<'a> IntoSql<'a> for Cow<'a, str> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(Some(self))
    }
}

impl<'a> IntoSql<'a> for Option<Cow<'a, str>> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::String(self)
    }
}

impl<'a> IntoSql<'a> for Cow<'a, [u8]> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(Some(self))
    }
}

impl<'a> IntoSql<'a> for Option<Cow<'a, [u8]>> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Binary(self)
    }
}

impl<'a> IntoSql<'a> for &'a XmlData {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Xml(Some(Cow::Borrowed(self)))
    }
}

impl<'a> IntoSql<'a> for Option<&'a XmlData> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Xml(self.map(Cow::Borrowed))
    }
}

impl<'a> IntoSql<'a> for &'a Uuid {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Guid(Some(*self))
    }
}

impl<'a> IntoSql<'a> for Option<&'a Uuid> {
    fn into_sql(self) -> ColumnData<'a> {
        ColumnData::Guid(self.copied())
    }
}

into_sql!(self_,
          String: (ColumnData::String, Cow::from(self_));
          Vec<u8>: (ColumnData::Binary, Cow::from(self_));
          Numeric: (ColumnData::Numeric, self_);
          XmlData: (ColumnData::Xml, Cow::Owned(self_));
          Uuid: (ColumnData::Guid, self_);
          bool: (ColumnData::Bit, self_);
          u8: (ColumnData::U8, self_);
          i16: (ColumnData::I16, self_);
          i32: (ColumnData::I32, self_);
          i64: (ColumnData::I64, self_);
          f32: (ColumnData::F32, self_);
          f64: (ColumnData::F64, self_);
);

to_sql!(self_,
        bool: (ColumnData::Bit, *self_);
        u8: (ColumnData::U8, *self_);
        i16: (ColumnData::I16, *self_);
        i32: (ColumnData::I32, *self_);
        i64: (ColumnData::I64, *self_);
        f32: (ColumnData::F32, *self_);
        f64: (ColumnData::F64, *self_);
        &str: (ColumnData::String, Cow::from(*self_));
        String: (ColumnData::String, Cow::from(self_));
        Cow<'_, str>: (ColumnData::String, self_.clone());
        &[u8]: (ColumnData::Binary, Cow::from(*self_));
        Cow<'_, [u8]>: (ColumnData::Binary, self_.clone());
        Vec<u8>: (ColumnData::Binary, Cow::from(self_));
        Numeric: (ColumnData::Numeric, *self_);
        XmlData: (ColumnData::Xml, Cow::Borrowed(self_));
        Uuid: (ColumnData::Guid, *self_);
);

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test for #404: a query-builder-style struct holding a
    // `&dyn ToSql` field couldn't derive `Debug`, since `ToSql` itself
    // doesn't require `Debug`.
    #[derive(Debug)]
    struct Param<'a> {
        value: &'a dyn ToSql,
    }

    #[test]
    fn struct_holding_dyn_to_sql_can_derive_debug() {
        let param = Param { value: &42i32 };
        assert_eq!(ColumnData::I32(Some(42)), param.value.to_sql());
        assert_eq!("Param { value: I32(Some(42)) }", format!("{:?}", param));
    }

    #[test]
    fn borrowed_into_sql_implementations_preserve_values_and_nulls() {
        let string = "hello".to_owned();
        let bytes = vec![1, 2, 3];
        let xml = XmlData::new("<x/>");
        let uuid = Uuid::from_bytes([7; 16]);

        assert_eq!(
            (&*string).into_sql(),
            ColumnData::String(Some("hello".into()))
        );
        assert_eq!(
            Some(&*string).into_sql(),
            ColumnData::String(Some("hello".into()))
        );
        assert_eq!(Option::<&str>::None.into_sql(), ColumnData::String(None));
        assert_eq!(
            (&string).into_sql(),
            ColumnData::String(Some("hello".into()))
        );
        assert_eq!(Option::<&String>::None.into_sql(), ColumnData::String(None));
        assert_eq!(
            (&bytes[..]).into_sql(),
            ColumnData::Binary(Some((&bytes[..]).into()))
        );
        assert_eq!(Option::<&[u8]>::None.into_sql(), ColumnData::Binary(None));
        assert_eq!(
            (&bytes).into_sql(),
            ColumnData::Binary(Some((&bytes[..]).into()))
        );
        assert_eq!(
            Option::<&Vec<u8>>::None.into_sql(),
            ColumnData::Binary(None)
        );
        assert_eq!(
            Cow::Borrowed("hello").into_sql(),
            ColumnData::String(Some("hello".into()))
        );
        assert_eq!(
            Option::<Cow<'_, str>>::None.into_sql(),
            ColumnData::String(None)
        );
        assert_eq!(
            Cow::Borrowed(&bytes[..]).into_sql(),
            ColumnData::Binary(Some((&bytes[..]).into()))
        );
        assert_eq!(
            Option::<Cow<'_, [u8]>>::None.into_sql(),
            ColumnData::Binary(None)
        );
        assert_eq!(
            (&xml).into_sql(),
            ColumnData::Xml(Some(Cow::Borrowed(&xml)))
        );
        assert_eq!(Option::<&XmlData>::None.into_sql(), ColumnData::Xml(None));
        assert_eq!((&uuid).into_sql(), ColumnData::Guid(Some(uuid)));
        assert_eq!(Option::<&Uuid>::None.into_sql(), ColumnData::Guid(None));
    }
}
