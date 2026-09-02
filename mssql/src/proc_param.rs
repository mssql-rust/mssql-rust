use crate::{tds::codec::ColumnData, ToSql};
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
        }
    }
}
