//! Checked numeric conversions.
//!
//! Implements the conversion rules of `spec/rust-checked-arithmetic`
//! (RCA-CONV-1, RCA-CONV-2): a conversion that can lose information is a
//! `TryFrom` whose failure becomes this crate's [`Error`], naming the figure
//! but never its value (RCA-ERR-2).

use crate::{error::Error, Result};

/// Converts `value` to `U`, or fails with [`Error::Conversion`] naming
/// `figure`. For figures the driver itself is about to *send* or compute
/// (a length that does not fit its wire field, for example).
pub(crate) fn convert<T, U>(value: T, figure: &'static str) -> Result<U>
where
    U: TryFrom<T>,
{
    U::try_from(value).map_err(|_| {
        Error::Conversion(format!("{figure}: value does not fit the target type").into())
    })
}

/// Converts `value` to `U`, or fails with [`Error::Protocol`] naming
/// `figure`. For figures *received* from the server: invalid wire data is
/// never recoverable (RCA-DECL-5).
pub(crate) fn convert_wire<T, U>(value: T, figure: &'static str) -> Result<U>
where
    U: TryFrom<T>,
{
    U::try_from(value).map_err(|_| {
        Error::Protocol(format!("{figure}: value does not fit the target type").into())
    })
}

/// The error for an arithmetic step on `figure` whose result does not fit
/// its type (RCA-P-4): names the figure, never the operands (RCA-ERR-2).
pub(crate) fn overflow(figure: &'static str) -> Error {
    Error::Conversion(format!("{figure}: arithmetic overflow").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_ok_at_the_limit() {
        assert_eq!(convert::<usize, u16>(65_535, "len").unwrap(), u16::MAX);
    }

    #[test]
    fn convert_names_the_figure_but_not_the_value() {
        let err = convert::<usize, u16>(65_536, "frame_len").unwrap_err();
        assert_eq!(
            err,
            Error::Conversion("frame_len: value does not fit the target type".into())
        );
    }

    #[test]
    fn convert_negative_to_unsigned_fails() {
        assert!(convert::<i32, u32>(-1, "x").is_err());
    }

    #[test]
    fn overflow_names_the_figure() {
        assert_eq!(
            overflow("row_length"),
            Error::Conversion("row_length: arithmetic overflow".into())
        );
    }

    #[test]
    fn convert_wire_is_a_protocol_error() {
        let err = convert_wire::<u32, u8>(256, "scale").unwrap_err();
        assert_eq!(
            err,
            Error::Protocol("scale: value does not fit the target type".into())
        );
        assert_eq!(convert_wire::<u32, u8>(255, "scale").unwrap(), 255);
    }
}
