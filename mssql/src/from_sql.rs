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
/// |`jiff::civil::DateTime` (with feature flag `jiff`)|`datetime`/`datetime2`/`smalldatetime`|
/// |`jiff::civil::Date` (with feature flag `jiff`)|`date`|
/// |`jiff::civil::Time` (with feature flag `jiff`)|`time`|
/// |`jiff::Timestamp` / `jiff::Zoned` (with feature flag `jiff`)|`datetimeoffset`|
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
        // Lossless widening of a `smallint` (upstream Tiberius 97bbbfd):
        // reading one as `i32` used to be a conversion error.
        ColumnData::I16(val) => (val.map(i32::from), val.map(i32::from)),
        ColumnData::U8(None) => (None, None),
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

#[cfg(test)]
mod widening_tests {
    use super::*;

    #[test]
    fn smallint_reads_as_i32() {
        assert_eq!(
            Some(8i32),
            i32::from_sql(&ColumnData::I16(Some(8))).unwrap()
        );
        assert_eq!(
            Some(-8i32),
            i32::from_sql_owned(ColumnData::I16(Some(-8))).unwrap()
        );
        assert_eq!(None, i32::from_sql(&ColumnData::I16(None)).unwrap());
    }

    #[test]
    fn primitive_conversions_cover_values_null_widths_and_type_errors() {
        assert_eq!(
            bool::from_sql(&ColumnData::Bit(Some(true))).unwrap(),
            Some(true)
        );
        assert_eq!(u8::from_sql(&ColumnData::U8(Some(7))).unwrap(), Some(7));
        assert_eq!(i16::from_sql(&ColumnData::I16(Some(-7))).unwrap(), Some(-7));
        assert_eq!(i64::from_sql(&ColumnData::I64(Some(-7))).unwrap(), Some(-7));
        assert_eq!(
            f32::from_sql(&ColumnData::F32(Some(1.5))).unwrap(),
            Some(1.5)
        );
        assert_eq!(
            f64::from_sql(&ColumnData::F64(Some(1.5))).unwrap(),
            Some(1.5)
        );

        for null in [
            ColumnData::U8(None),
            ColumnData::I16(None),
            ColumnData::I32(None),
            ColumnData::I64(None),
        ] {
            assert_eq!(u8::from_sql(&null).unwrap(), None);
            assert_eq!(i16::from_sql(&null).unwrap(), None);
            assert_eq!(i32::from_sql(&null).unwrap(), None);
            assert_eq!(i64::from_sql(&null).unwrap(), None);
        }
        assert_eq!(f32::from_sql(&ColumnData::F64(None)).unwrap(), None);
        assert_eq!(f64::from_sql(&ColumnData::F32(None)).unwrap(), None);
        assert!(bool::from_sql(&ColumnData::I32(Some(1))).is_err());
    }

    #[test]
    fn borrowed_and_owned_lob_conversions_cover_success_and_errors() {
        let string = ColumnData::String(Some("hello".into()));
        assert_eq!(<&str>::from_sql(&string).unwrap(), Some("hello"));
        assert_eq!(
            String::from_sql_owned(string).unwrap().as_deref(),
            Some("hello")
        );

        let binary = ColumnData::Binary(Some(vec![1, 2, 3].into()));
        assert_eq!(<&[u8]>::from_sql(&binary).unwrap(), Some(&[1, 2, 3][..]));
        assert_eq!(
            Vec::<u8>::from_sql_owned(binary).unwrap(),
            Some(vec![1, 2, 3])
        );

        let xml = ColumnData::Xml(Some(std::borrow::Cow::Owned(XmlData::new("<x/>"))));
        assert_eq!(
            <&XmlData>::from_sql(&xml).unwrap().unwrap().as_ref(),
            "<x/>"
        );
        assert_eq!(
            XmlData::from_sql_owned(xml).unwrap().unwrap().as_ref(),
            "<x/>"
        );

        assert!(<&str>::from_sql(&ColumnData::I32(Some(1))).is_err());
        assert!(String::from_sql_owned(ColumnData::I32(Some(1))).is_err());
        assert!(<&[u8]>::from_sql(&ColumnData::I32(Some(1))).is_err());
        assert!(Vec::<u8>::from_sql_owned(ColumnData::I32(Some(1))).is_err());
        assert!(<&XmlData>::from_sql(&ColumnData::I32(Some(1))).is_err());
        assert!(XmlData::from_sql_owned(ColumnData::I32(Some(1))).is_err());
    }
}
