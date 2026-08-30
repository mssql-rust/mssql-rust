use crate::{tds::Numeric, xml::XmlData, ColumnData};
use uuid::Uuid;

/// A conversion trait from a TDS type by-reference.
///
/// A `FromSql` implementation for a Rust type is needed for using it as a
/// return parameter from [`Row::get`](crate::Row::get) or
/// [`Row::try_get`](crate::Row::try_get) methods. The following Rust types
/// are already implemented to match the given server types:
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
/// |`String`/`&str`|`nvarchar`/`varchar`/`nchar`/`char`/`ntext`/`text`|
/// |`Vec<u8>`/`&[u8]`|`binary`/`varbinary`/`image`|
/// |[`Uuid`]|`uniqueidentifier`|
/// |[`Numeric`](crate::numeric::Numeric)|`numeric`/`decimal`|
/// |`Decimal` (with feature flag `rust_decimal`, see [`crate::numeric`])|`numeric`/`decimal`|
/// |[`XmlData`](crate::xml::XmlData)|`xml`|
/// |`NaiveDateTime` (with feature flag `chrono`)|`datetime`/`datetime2`/`smalldatetime`|
/// |`NaiveDate` (with feature flag `chrono`)|`date`|
/// |`NaiveTime` (with feature flag `chrono`)|`time`|
/// |`DateTime` (with feature flag `chrono`)|`datetimeoffset`|
///
/// See the [`time`](crate::time) module for more information about the date
/// and time structs.
pub trait FromSql<'a>
where
    Self: Sized + 'a,
{
    /// Returns the value, `None` being a null value, copying the value.
    fn from_sql(value: &'a ColumnData<'static>) -> crate::Result<Option<Self>>;
}

/// A conversion trait from a TDS type by-value.
pub trait FromSqlOwned
where
    Self: Sized,
{
    /// Returns the value, `None` being a null value, taking the ownership.
    fn from_sql_owned(value: ColumnData<'static>) -> crate::Result<Option<Self>>;
}

from_sql!(bool: ColumnData::Bit(val) => (*val, val));
// A NULL integer column decodes to whichever `ColumnData` variant matches
// its own declared width (e.g. a NULL `smallint` is always `I16(None)`,
// never `I32(None)`), regardless of which width the caller reads it back
// as. Since a NULL carries no value to actually widen or narrow, every
// integer width's `FromSql` accepts every other width's `None` variant as
// `None` too - not just the ones a previous fix happened to add (#263).
from_sql!(
    u8:
        ColumnData::U8(val) => (*val, val),
        ColumnData::I16(None) => (None, None),
        ColumnData::I32(None) => (None, None),
        ColumnData::I64(None) => (None, None)
);
from_sql!(
    i16:
        ColumnData::I16(val) => (*val, val),
        ColumnData::U8(None) => (None, None),
        ColumnData::I32(None) => (None, None),
        ColumnData::I64(None) => (None, None)
);
from_sql!(
    i32:
        ColumnData::I32(val) => (*val, val),
        ColumnData::U8(None) => (None, None),
        ColumnData::I16(None) => (None, None),
        ColumnData::I64(None) => (None, None)
);
from_sql!(
    i64:
        ColumnData::I64(val) => (*val, val),
        ColumnData::U8(None) => (None, None),
        ColumnData::I16(None) => (None, None),
        ColumnData::I32(None) => (None, None)
);
// Same reasoning for the two floating-point widths: a NULL `float(24)` is
// always `F32(None)`, a NULL `float(53)` always `F64(None)`.
from_sql!(f32: ColumnData::F32(val) => (*val, val), ColumnData::F64(None) => (None, None));
from_sql!(f64: ColumnData::F64(val) => (*val, val), ColumnData::F32(None) => (None, None));
from_sql!(Uuid: ColumnData::Guid(val) => (*val, val));
from_sql!(Numeric: ColumnData::Numeric(n) => (*n, n));

impl FromSqlOwned for XmlData {
    fn from_sql_owned(value: ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::Xml(data) => Ok(data.map(|data| data.into_owned())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a String value", v).into(),
            )),
        }
    }
}

impl<'a> FromSql<'a> for &'a XmlData {
    fn from_sql(value: &'a ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::Xml(data) => Ok(data.as_ref().map(|s| s.as_ref())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a String value", v).into(),
            )),
        }
    }
}

impl FromSqlOwned for String {
    fn from_sql_owned(value: ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::String(s) => Ok(s.map(|s| s.into_owned())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a String value", v).into(),
            )),
        }
    }
}

impl<'a> FromSql<'a> for &'a str {
    fn from_sql(value: &'a ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::String(s) => Ok(s.as_ref().map(|s| s.as_ref())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a String value", v).into(),
            )),
        }
    }
}

impl FromSqlOwned for Vec<u8> {
    fn from_sql_owned(value: ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::Binary(b) => Ok(b.map(|s| s.into_owned())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a String value", v).into(),
            )),
        }
    }
}

impl<'a> FromSql<'a> for &'a [u8] {
    fn from_sql(value: &'a ColumnData<'static>) -> crate::Result<Option<Self>> {
        match value {
            ColumnData::Binary(b) => Ok(b.as_ref().map(|s| s.as_ref())),
            v => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a &[u8] value", v).into(),
            )),
        }
    }
}
