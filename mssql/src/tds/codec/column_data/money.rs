use crate::{error::Error, sql_read_bytes::SqlReadBytes, ColumnData};

pub(crate) async fn decode<R>(src: &mut R, len: u8) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let res = match len {
        0 => ColumnData::F64(None),
        4 => ColumnData::F64(Some(f64::from(src.read_i32_le().await?) / 1e4)),
        8 => ColumnData::F64(Some({
            let high = f64::from(src.read_i32_le().await?);
            let low = f64::from(src.read_u32_le().await?);

            // `high` is a 32-bit value, so `high * 2^32` is exact in an f64
            // (RCA-CONV-4); only the final sum rounds, once.
            (high * 4_294_967_296.0 + low) / 1e4
        })),
        _ => {
            return Err(Error::Protocol(
                format!("money: length of {} is invalid", len).into(),
            ))
        }
    };

    Ok(res)
}

// Encoding (bulk insert). Ported from upstream Tiberius
// (tiberius-rs/tiberius 302cfe4, e0a65d8, 1d91e18 and 8510d35, by Matthew
// Jackson). Thanks to the Tiberius team.
//
// money/smallmoney are fixed-point integers scaled by 10^4: i64 for money
// (sent as high i32 then low u32) and i32 for smallmoney. A nullable column
// (`VarLenSized(Money)`, MONEYN) prefixes the value with a length byte; a
// NOT NULL column (`FixedLen(Money/Money4)`) takes the raw bytes.

const MONEY_SCALE: u32 = 4;

/// Range-checks an already-scaled value (held in i128, so nothing has
/// wrapped yet) against the column width and writes it.
fn put_scaled(
    dst: &mut impl bytes::BufMut,
    len: usize,
    scaled: i128,
    shown: &dyn std::fmt::Display,
    length_prefixed: bool,
) -> crate::Result<()> {
    match len {
        4 => {
            let v = i32::try_from(scaled).map_err(|_| {
                Error::BulkInput(format!("value {shown} out of range for smallmoney").into())
            })?;
            if length_prefixed {
                dst.put_u8(4);
            }
            dst.put_i32_le(v);
        }
        8 => {
            let v = i64::try_from(scaled).map_err(|_| {
                Error::BulkInput(format!("value {shown} out of range for money").into())
            })?;
            if length_prefixed {
                dst.put_u8(8);
            }
            // Bit-pattern split (RCA-BIT-2) into the wire's two 32-bit
            // halves: the high i32, then the low u32.
            let [l0, l1, l2, l3, h0, h1, h2, h3] = v.to_le_bytes();
            dst.put_i32_le(i32::from_le_bytes([h0, h1, h2, h3]));
            dst.put_u32_le(u32::from_le_bytes([l0, l1, l2, l3]));
        }
        len => {
            return Err(Error::BulkInput(
                format!("money: invalid column length {len}").into(),
            ))
        }
    }
    Ok(())
}

/// Encodes an `f64`. It used to be `(val * 1e4).round() as i64`, which
/// silently saturates: NaN became 0 and infinities or out-of-range values
/// became i64::MIN/MAX. Those are now errors. The range check is done in
/// i128, since `i64::MAX as f64` rounds up to 2^63 and would let the boundary
/// value through.
pub(crate) fn encode_f64(
    dst: &mut impl bytes::BufMut,
    len: usize,
    val: f64,
    length_prefixed: bool,
) -> crate::Result<()> {
    if !val.is_finite() {
        return Err(Error::BulkInput(
            format!("money: value {val} is not finite").into(),
        ));
    }
    let scaled_f = (val * 1e4).round();
    // RCA-CONV-3: only a finite value below 2^63 in magnitude is converted;
    // anything larger is out of range for money (i64) and smallmoney (i32)
    // alike, and `val * 1e4` can overflow to infinity even when `val` is
    // finite.
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    if !(scaled_f.is_finite() && scaled_f.abs() < TWO_POW_63) {
        let column = if len == 4 { "smallmoney" } else { "money" };
        return Err(Error::BulkInput(
            format!("value {val} out of range for {column}").into(),
        ));
    }
    #[expect(
        clippy::as_conversions,
        reason = "RCA-CONV-3: scaled_f is finite and below 2^63 in magnitude (checked above), and already rounded to an integral value"
    )]
    let scaled = scaled_f as i128;
    put_scaled(dst, len, scaled, &val, length_prefixed)
}

/// Encodes a `Numeric` exactly, in integer arithmetic. Going through `f64`
/// loses precision past 2^53, well inside money's ~9.2e18 scaled range. A
/// scale finer than 4 rounds half away from zero, like the `f64` path.
pub(crate) fn encode_numeric(
    dst: &mut impl bytes::BufMut,
    len: usize,
    num: crate::tds::Numeric,
    length_prefixed: bool,
) -> crate::Result<()> {
    let scale = u32::from(num.scale());
    let mantissa = num.value();
    let overflow = || Error::BulkInput(format!("value {num} out of range for money").into());

    let scaled = if scale <= MONEY_SCALE {
        let factor = MONEY_SCALE
            .checked_sub(scale)
            .and_then(|digits| 10i128.checked_pow(digits))
            .ok_or_else(overflow)?;
        mantissa.checked_mul(factor).ok_or_else(overflow)?
    } else {
        let divisor = scale
            .checked_sub(MONEY_SCALE)
            .and_then(|digits| 10i128.checked_pow(digits))
            .ok_or_else(overflow)?;
        let quotient = mantissa.checked_div(divisor).ok_or_else(overflow)?;
        let remainder = mantissa
            .checked_rem(divisor)
            .ok_or_else(overflow)?
            .unsigned_abs();
        let doubled = remainder.checked_mul(2).ok_or_else(overflow)?;
        if doubled >= divisor.unsigned_abs() {
            quotient
                .checked_add(mantissa.signum())
                .ok_or_else(overflow)?
        } else {
            quotient
        }
    };

    put_scaled(dst, len, scaled, &num, length_prefixed)
}

#[cfg(test)]
mod encode_tests {
    use super::*;
    use crate::tds::Numeric;
    use bytes::BytesMut;

    fn f64_bytes(len: usize, val: f64, prefixed: bool) -> crate::Result<Vec<u8>> {
        let mut buf = BytesMut::new();
        encode_f64(&mut buf, len, val, prefixed)?;
        Ok(buf.to_vec())
    }

    #[test]
    fn non_finite_and_out_of_range_are_errors_not_saturation() {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e15, -1e15] {
            assert!(f64_bytes(8, v, true).is_err(), "money {v}");
        }
        for v in [214748.3648, -214748.3649, 1e10] {
            assert!(f64_bytes(4, v, true).is_err(), "smallmoney {v}");
        }
        // 2^63 / 10^4 rounds to exactly 2^63 once scaled: one past i64::MAX.
        assert!(f64_bytes(8, 922337203685477.6, true).is_err());
    }

    #[test]
    fn framing_with_and_without_length_prefix() {
        assert_eq!(f64_bytes(4, 1.5, true).unwrap(), [4, 0x98, 0x3a, 0, 0]);
        assert_eq!(f64_bytes(4, 1.5, false).unwrap(), [0x98, 0x3a, 0, 0]);
        assert_eq!(
            f64_bytes(8, 1.5, false).unwrap(),
            [0, 0, 0, 0, 0x98, 0x3a, 0, 0]
        );
    }

    #[test]
    fn numeric_is_exact_and_rounds_half_away_from_zero() {
        let mut a = BytesMut::new();
        // Money's max, which f64 cannot represent exactly.
        encode_numeric(
            &mut a,
            8,
            Numeric::new_with_scale(9_223_372_036_854_775_807, 4),
            false,
        )
        .unwrap();
        let mut b = BytesMut::new();
        put_scaled(&mut b, 8, i64::MAX as i128, &"", false).unwrap();
        assert_eq!(a, b);

        for (value, scale, expected) in [
            (123_456, 5, 12_346i32),
            (123_451, 5, 12_345),
            (-123_455, 5, -12_346),
            (1, 0, 10_000),
        ] {
            let mut x = BytesMut::new();
            encode_numeric(&mut x, 4, Numeric::new_with_scale(value, scale), false).unwrap();
            assert_eq!(x.as_ref(), expected.to_le_bytes(), "{value}e-{scale}");
        }

        let mut x = BytesMut::new();
        assert!(encode_numeric(&mut x, 8, Numeric::new_with_scale(i128::MAX, 0), true).is_err());
        assert!(f64_bytes(1, 1.0, false).is_err());
    }
}
