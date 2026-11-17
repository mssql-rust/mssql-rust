use crate::{tds::codec::ColumnData, ToSql, TokenRow};
use std::borrow::Cow;

/// A single parameter to [`Client::call_procedure`](crate::Client::call_procedure),
/// naming both its value and (for an OUTPUT parameter) that the server
/// should send a value back.
///
/// The name must include its leading `@`, matching how the parameter is
/// declared in the procedure's own signature (e.g. `"@out"`, not `"out"`).
#[derive(Debug)]
pub struct ProcParam<'a> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) value: ColumnData<'a>,
    pub(crate) output: bool,
    pub(crate) table: Option<TableParam<'a>>,
}

/// The rows and table type of a [`ProcParam::table`] argument.
#[derive(Debug, Clone)]
pub(crate) struct TableParam<'a> {
    pub(crate) type_name: Cow<'a, str>,
    pub(crate) rows: Vec<TokenRow<'a>>,
}

impl<'a> ProcParam<'a> {
    /// A plain input parameter - the common case, equivalent to how
    /// [`Client::query`](crate::Client::query)/[`execute`](crate::Client::execute)
    /// already take their parameters.
    pub fn input(name: impl Into<Cow<'a, str>>, value: &'a dyn ToSql) -> Self {
        Self {
            name: name.into(),
            value: value.to_sql(),
            output: false,
            table: None,
        }
    }

    /// An OUTPUT parameter. `value` establishes the SQL type (and an
    /// initial value the procedure almost always ignores) sent to the
    /// server - a concrete, non-`NULL` placeholder of the right type (e.g.
    /// `&0i32` for an `OUTPUT INT` parameter), not `&None::<i32>`: unlike
    /// [`input`](Self::input), the server needs to learn the type from this
    /// value alone, and rejects an untyped `NULL` bound as `OUTPUT`
    /// ([`Client::call_procedure`](crate::Client::call_procedure) checks
    /// for this client-side with a clear error). The procedure's own value
    /// for it is read back from
    /// [`QueryStream::into_output_params`](crate::QueryStream::into_output_params),
    /// not from this parameter itself.
    pub fn output(name: impl Into<Cow<'a, str>>, value: &'a dyn ToSql) -> Self {
        Self {
            name: name.into(),
            value: value.to_sql(),
            output: true,
            table: None,
        }
    }

    /// A table-valued parameter: `rows` sent as a value of the user-defined
    /// table type `type_name` (`"MyType"` or `"schema.MyType"`; brackets
    /// such as `"[my schema].[My Type]"` work too). Each row must have one
    /// value per column of the type, in the type's column order. The
    /// procedure must declare the parameter `READONLY`.
    ///
    /// `call_procedure` asks the server for the type's columns first, using
    /// one extra round trip per table parameter.
    ///
    /// Ported and reworked from upstream Tiberius (tiberius-rs/tiberius
    /// 73e79d4), without its derive macro and with the type name quoted
    /// rather than interpolated.
    pub fn table(
        name: impl Into<Cow<'a, str>>,
        type_name: impl Into<Cow<'a, str>>,
        rows: Vec<TokenRow<'a>>,
    ) -> Self {
        Self {
            name: name.into(),
            value: ColumnData::I32(None),
            output: false,
            table: Some(TableParam {
                type_name: type_name.into(),
                rows,
            }),
        }
    }
}
