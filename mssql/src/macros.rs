macro_rules! uint_enum {
    ($( #[$gattr:meta] )* pub enum $ty:ident { $( $( #[$attr:meta] )* $variant:ident = $val:expr,)* }) => {
        uint_enum!($( #[$gattr ])* (pub) enum $ty { $( $( #[$attr] )* $variant = $val, )* });
    };
    ($( #[$gattr:meta] )* enum $ty:ident { $( $( #[$attr:meta] )* $variant:ident = $val:expr,)* }) => {
        #[allow(missing_docs)]
        uint_enum!($( #[$gattr ])* () enum $ty { $( $( #[$attr] )* $variant = $val, )* });
    };

    ($( #[$gattr:meta] )* ( $($vis:tt)* ) enum $ty:ident { $( $( #[$attr:meta] )* $variant:ident = $val:expr,)* }) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        $( #[$gattr] )*
        #[allow(missing_docs)]
        $( $vis )* enum $ty {
            $( $( #[$attr ])* $variant = $val, )*
        }

        impl $ty {
            #[expect(
                clippy::as_conversions,
                reason = "RCA-EXC-2: fieldless enum whose discriminants are at most 32 bits wide; the cast returns the declared discriminant exactly"
            )]
            const fn discriminant(self) -> u32 {
                self as u32
            }

            /// Lowest byte of the discriminant. Some enums using this macro
            /// (e.g. FeatureLevel) have discriminants wider than u8; the
            /// truncation to one byte is intentional: matching input `n`
            /// never accidentally succeeds against a differently-valued
            /// variant.
            const fn low_byte(self) -> u8 {
                let [low, _, _, _] = self.discriminant().to_le_bytes();
                low
            }
        }

        impl ::std::convert::TryFrom<u8> for $ty {
            type Error = ();
            fn try_from(n: u8) -> ::std::result::Result<$ty, ()> {
                match n {
                    $( x if x == $ty::$variant.low_byte() => Ok($ty::$variant), )*
                    _ => Err(()),
                }
            }
        }

        impl ::std::convert::TryFrom<u32> for $ty {
            type Error = ();
            fn try_from(n: u32) -> ::std::result::Result<$ty, ()> {
                match n {
                    $( x if x == $ty::$variant.discriminant() => Ok($ty::$variant), )*
                    _ => Err(()),
                }
            }
        }
    }
}

/// `impl From<$ty> for $int` for a fieldless enum whose discriminants fit `$int`: the one
/// place its discriminant is read (RCA-CONV-1), so no call site needs `as`.
macro_rules! enum_into_int {
    ($ty:ty => $int:ty) => {
        impl ::std::convert::From<$ty> for $int {
            fn from(value: $ty) -> $int {
                // RCA-EXC-2: fieldless enum whose declared discriminants all
                // fit the target integer type, so the cast returns the
                // declared discriminant exactly. (Clippy's `as_conversions`
                // does not report a cast whose target type comes from a macro
                // argument, so no `#[expect]` can record it; this comment
                // and the per-enum tests do.)
                value as $int
            }
        }
    };
}

macro_rules! to_sql {
    ($target:ident, $( $ty:ty: ($variant:expr, $val:expr) ;)* ) => {
        $(
            impl crate::ToSql for $ty {
                fn to_sql(&self) -> crate::tds::codec::ColumnData<'_> {
                    let $target = self;
                    $variant(Some($val))
                }
            }

            impl crate::ToSql for Option<$ty> {
                fn to_sql(&self) -> crate::tds::codec::ColumnData<'_> {
                    match self {
                        Some(val) => {
                            let $target = val;
                            $variant(Some($val))
                        },
                        None => $variant(None)
                    }
                }
            }

            impl crate::ToSql for &Option<$ty> {
                fn to_sql(&self) -> crate::tds::codec::ColumnData<'_> {
                    match self {
                        Some(val) => {
                            let $target = val;
                            $variant(Some($val))
                        },
                        None => $variant(None)
                    }
                }
            }
        )*
    };
}

macro_rules! into_sql {
    ($target:ident, $( $ty:ty: ($variant:expr, $val:expr) ;)* ) => {
        $(
            impl<'a> crate::IntoSql<'a> for $ty {
                fn into_sql(self) -> crate::tds::codec::ColumnData<'a> {
                    let $target = self;
                    $variant(Some($val))
                }
            }

            impl<'a> crate::IntoSql<'a> for Option<$ty> {
                fn into_sql(self) -> crate::tds::codec::ColumnData<'a> {
                    match self {
                        Some(val) => {
                            let $target = val;
                            $variant(Some($val))
                        },
                        None => $variant(None)
                    }
                }
            }
        )*
    }
}

macro_rules! from_sql {
    ($( $ty:ty: $($pat:pat => ($borrowed_val:expr, $owned_val:expr)),* );* ) => {
        $(
            impl<'a> crate::FromSql<'a> for $ty {
                fn from_sql(data: &'a crate::tds::codec::ColumnData<'static>) -> crate::Result<Option<Self>> {
                    match data {
                        $( $pat => Ok($borrowed_val), )*
                        _ => Err(crate::Error::Conversion(format!("cannot interpret {:?} as an {} value", data, stringify!($ty)).into()))
                    }
                }
            }

            impl crate::FromSqlOwned for $ty {
                fn from_sql_owned(data: crate::tds::codec::ColumnData<'static>) -> crate::Result<Option<Self>> {
                    match data {
                        $( $pat => Ok($owned_val), )*
                        _ => Err(crate::Error::Conversion(format!("cannot interpret {:?} as an {} value", data, stringify!($ty)).into()))
                    }
                }
            }
        )*
    };
    ($( $ty:ty: $($pat:pat => $borrowed_val:expr),* );* ) => {
        $(
            impl<'a> crate::FromSql<'a> for $ty {
                fn from_sql(data: &'a crate::tds::codec::ColumnData<'static>) -> crate::Result<Option<Self>> {
                    match data {
                        $( $pat => Ok($borrowed_val), )*
                        _ => Err(crate::Error::Conversion(format!("cannot interpret {:?} as an {} value", data, stringify!($ty)).into()))
                    }
                }
            }

            impl crate::FromSqlOwned for $ty {
                fn from_sql_owned(data: crate::tds::codec::ColumnData<'static>) -> crate::Result<Option<Self>> {
                    match data {
                        $( $pat => Ok($borrowed_val), )*
                        _ => Err(crate::Error::Conversion(format!("cannot interpret {:?} as an {} value", data, stringify!($ty)).into()))
                    }
                }
            }
        )*
    };
}
