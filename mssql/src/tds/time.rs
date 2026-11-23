//! Date and time handling.
//!
//! When using the `tds73` feature flag together with SQL Server 2008 or later,
//! the following `time` crate mappings to and from the database are available
//! (re-exported from the `time` submodule, enabled by the `time` feature):
//!
//! - `Time` -> `Time`
//! - `Date` -> `Date`
//! - `DateTime` -> `PrimitiveDateTime`
//! - `DateTime2` -> `PrimitiveDateTime`
//! - `SmallDateTime` -> `PrimitiveDateTime`
//! - `DateTimeOffset` -> `OffsetDateTime`
//!
//! With SQL Server 2005 and the `tds73` feature flag disabled, the mapping is
//! different:
//!
//! - `DateTime` -> `PrimitiveDateTime`
//! - `SmallDateTime` -> `PrimitiveDateTime`

#[cfg(feature = "jiff")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "jiff")))]
pub mod jiff;

// Named after the `time` crate it maps to/from, matching the `jiff`
// submodule above; not an accidental duplication of the parent module name.
#[allow(clippy::module_inception)]
#[cfg(feature = "time")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "time")))]
pub mod time;

#[cfg(any(feature = "jiff", feature = "time"))]
use crate::tds::codec::ColumnData;
use crate::{tds::codec::Encode, SqlReadBytes};
#[cfg(feature = "tds73")]
use byteorder::{ByteOrder, LittleEndian};
use bytes::{BufMut, BytesMut};
#[cfg(feature = "tds73")]
use futures_util::io::AsyncReadExt;

/// A presentation of `datetime` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. For dealing with `datetime`,
/// use the `time` feature of this crate and its `PrimitiveDateTime` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DateTime {
    days: i32,
    seconds_fragments: u32,
}

impl DateTime {
    /// Construct a new `DateTime` instance.
    pub fn new(days: i32, seconds_fragments: u32) -> Self {
        Self {
            days,
            seconds_fragments,
        }
    }

    /// Days since 1st of January, 1900 (including the negative range until 1st
    /// of January, 1753).
    pub fn days(self) -> i32 {
        self.days
    }

    /// 1/300 of a second, so a value of 300 equals 1 second (since midnight).
    pub fn seconds_fragments(self) -> u32 {
        self.seconds_fragments
    }

    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let days = src.read_i32_le().await?;
        let seconds_fragments = src.read_u32_le().await?;

        Ok(Self {
            days,
            seconds_fragments,
        })
    }
}

impl Encode<BytesMut> for DateTime {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_i32_le(self.days);
        dst.put_u32_le(self.seconds_fragments);

        Ok(())
    }
}

/// A presentation of `smalldatetime` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. For dealing with
/// `smalldatetime`, use the `time` feature of this crate and its
/// `PrimitiveDateTime` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SmallDateTime {
    days: u16,
    seconds_fragments: u16,
}

impl SmallDateTime {
    /// Construct a new `SmallDateTime` instance.
    pub fn new(days: u16, seconds_fragments: u16) -> Self {
        Self {
            days,
            seconds_fragments,
        }
    }
    /// Days since 1st of January, 1900.
    pub fn days(self) -> u16 {
        self.days
    }

    /// 1/300 of a second, so a value of 300 equals 1 second (since midnight)
    pub fn seconds_fragments(self) -> u16 {
        self.seconds_fragments
    }

    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let days = src.read_u16_le().await?;
        let seconds_fragments = src.read_u16_le().await?;

        Ok(Self {
            days,
            seconds_fragments,
        })
    }
}

impl Encode<BytesMut> for SmallDateTime {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        dst.put_u16_le(self.days);
        dst.put_u16_le(self.seconds_fragments);

        Ok(())
    }
}

/// A presentation of `date` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. If you want to deal with
/// `date`, use the `time` feature of this crate and its `Date` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
pub struct Date(u32);

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Date {
    #[inline]
    /// Construct a new `Date`
    ///
    /// # Panics
    /// max value of 3 bytes (`u32::max_value() > 8`). The `ToSql`/`IntoSql`
    /// conversions never reach this: a date outside 0001-01-01..9999-12-31
    /// becomes [`ColumnData::Invalid`](crate::ColumnData::Invalid) instead.
    pub fn new(days: u32) -> Date {
        assert_eq!(days >> 24, 0);
        Date(days)
    }

    #[inline]
    /// The number of days from 1st of January, year 1.
    pub fn days(self) -> u32 {
        self.0
    }

    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let mut bytes = [0u8; 4];
        src.read_exact(&mut bytes[..3]).await?;
        Ok(Self::new(LittleEndian::read_u32(&bytes)))
    }
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Encode<BytesMut> for Date {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        let mut tmp = [0u8; 4];
        LittleEndian::write_u32(&mut tmp, self.days());
        assert_eq!(tmp[3], 0);
        dst.extend_from_slice(&tmp[0..3]);

        Ok(())
    }
}

/// A presentation of `time` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. If you want to deal with
/// `time`, use the `time` feature of this crate and its `Time` type.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
pub struct Time {
    increments: u64,
    scale: u8,
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl PartialEq for Time {
    fn eq(&self, t: &Time) -> bool {
        // Compare exactly, in integers, at the finer of the two scales. An
        // increment count that does not fit u128 at that scale is out of
        // range, so it is never equal.
        let scale = self.scale.max(t.scale);
        match (self.increments_at(scale), t.increments_at(scale)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }
}

/// Minutes of a UTC offset given in seconds. Both date/time crates bound an
/// offset to +-25:59:59 (93 599 s), so the result is within +-1559 minutes.
#[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
pub(crate) fn offset_minutes(seconds: i32) -> i16 {
    #[expect(
        clippy::as_conversions,
        reason = "RCA-EXC-2: jiff and time both bound a UTC offset to +-93599 s, so seconds / 60 is within +-1559, well inside i16"
    )]
    let minutes = (seconds / 60) as i16;
    minutes
}

/// A day count since 0001-01-01 as the 3-byte wire [`Date`].
///
/// A negative day count (a date before 0001-01-01) or one past 2^24 - 1
/// (after 9999-12-31) is not representable as a `date`/`datetime2`; it is an
/// `Error::Conversion`, never a panic and never a substituted date.
#[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
pub(crate) fn try_date_from_days(days: i64) -> crate::Result<Date> {
    match u32::try_from(days) {
        Ok(days) if days < (1 << 24) => Ok(Date::new(days)),
        _ => Err(crate::Error::Conversion(
            "date out of range for SQL Server date (0001-01-01..9999-12-31)".into(),
        )),
    }
}

/// Turns the result of a fallible `ToSql`/`IntoSql` conversion into a
/// [`ColumnData`]: the value in `ctor` on success, [`ColumnData::Invalid`]
/// carrying the error message on failure. `ToSql` cannot return an error, and
/// a value must never be clamped or substituted, so the failure travels in the
/// parameter and is rejected before anything is sent.
#[cfg(any(feature = "jiff", feature = "time"))]
pub(crate) fn fallible<'a, T>(
    ctor: fn(Option<T>) -> ColumnData<'a>,
) -> impl Fn(Option<crate::Result<T>>) -> ColumnData<'a> {
    move |value| match value {
        None => ctor(None),
        Some(Ok(value)) => ctor(Some(value)),
        Some(Err(error)) => ColumnData::invalid(error),
    }
}

/// Nanoseconds since midnight from the clock fields of a time of day. Total:
/// the fields are bounded (hour < 24, minute and second < 60, nanosecond <
/// 10^9), so the sum is below 86 400 * 10^9, far inside u64.
#[cfg(feature = "time")]
pub(crate) fn nanos_of_day(hour: u8, minute: u8, second: u8, nanosecond: u32) -> u64 {
    u64::from(hour) * 3_600_000_000_000
        + u64::from(minute) * 60_000_000_000
        + u64::from(second) * 1_000_000_000
        + u64::from(nanosecond)
}

/// A legacy `datetime` day count (days since 1900-01-01) after range
/// checking: SQL Server's `datetime` spans 1753-01-01 (-53 690) to 9999-12-31
/// (2 958 463).
#[cfg(all(not(feature = "tds73"), any(feature = "jiff", feature = "time")))]
pub(crate) fn try_legacy_days(days: i64) -> crate::Result<i32> {
    match i32::try_from(days) {
        Ok(days) if (-53_690..=2_958_463).contains(&days) => Ok(days),
        _ => Err(crate::Error::Conversion(
            "date out of range for SQL Server datetime (1753-01-01..9999-12-31)".into(),
        )),
    }
}

/// A legacy `datetime` second-fraction count (1/300 s since midnight), from
/// nanoseconds since midnight (always below 86 400 * 10^9).
#[cfg(all(not(feature = "tds73"), any(feature = "jiff", feature = "time")))]
pub(crate) fn legacy_fragments(nanos_since_midnight: u64) -> u32 {
    #[expect(
        clippy::as_conversions,
        reason = "RCA-EXC-2: nanos < 86_400 * 10^9, so nanos * 300 / 10^9 < 25_920_000, inside u32"
    )]
    let fragments = (nanos_since_midnight * 300 / 1_000_000_000) as u32;
    fragments
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Time {
    /// `increments` re-expressed at `scale` (which must be at least this
    /// time's own scale), or `None` if that does not fit.
    fn increments_at(&self, scale: u8) -> Option<u128> {
        let factor = 10u128.checked_pow(u32::from(scale.checked_sub(self.scale)?))?;
        u128::from(self.increments).checked_mul(factor)
    }

    /// Nanoseconds since midnight: `increments` of 10^-`scale` second.
    /// A scale above 9 or an overflowing count is out of range.
    #[cfg(any(feature = "jiff", feature = "time"))]
    pub(crate) fn nanoseconds(self) -> crate::Result<u64> {
        9u8.checked_sub(self.scale)
            .and_then(|digits| 10u64.checked_pow(u32::from(digits)))
            .and_then(|factor| self.increments.checked_mul(factor))
            .ok_or_else(|| crate::num::overflow("time_increments"))
    }

    /// Construct a new `Time`
    pub fn new(increments: u64, scale: u8) -> Self {
        Self { increments, scale }
    }

    #[inline]
    /// Number of 10^-n second increments since midnight, where `n` is defined
    /// in [`scale`].
    ///
    /// [`scale`]: #method.scale
    pub fn increments(self) -> u64 {
        self.increments
    }

    #[inline]
    /// The accuracy of the increments.
    pub fn scale(self) -> u8 {
        self.scale
    }

    #[inline]
    /// Length of the field in number of bytes.
    pub(crate) fn len(self) -> crate::Result<u8> {
        Ok(match self.scale {
            0..=2 => 3,
            3..=4 => 4,
            5..=7 => 5,
            _ => {
                return Err(crate::Error::Protocol(
                    format!("timen: invalid scale {}", self.scale).into(),
                ))
            }
        })
    }

    pub(crate) async fn decode<R>(src: &mut R, n: usize, rlen: usize) -> crate::Result<Time>
    where
        R: SqlReadBytes + Unpin,
    {
        let val = match (n, rlen) {
            (0..=2, 3) => {
                let hi = u64::from(src.read_u16_le().await?);
                let lo = u64::from(src.read_u8().await?);

                hi | lo << 16
            }
            (3..=4, 4) => u64::from(src.read_u32_le().await?),
            (5..=7, 5) => {
                let hi = u64::from(src.read_u32_le().await?);
                let lo = u64::from(src.read_u8().await?);

                hi | lo << 32
            }
            _ => {
                return Err(crate::Error::Protocol(
                    format!("timen: invalid length {}", n).into(),
                ))
            }
        };

        Ok(Time {
            increments: val,
            scale: crate::num::convert_wire(n, "time_scale")?,
        })
    }
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Encode<BytesMut> for Time {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        // The increments are written little-endian in the 3, 4 or 5 bytes
        // the scale calls for (a bit-field encoding, RCA-BIT-2); any set bit
        // above that width means the value does not fit its wire field.
        let [b0, b1, b2, b3, b4, b5, b6, b7] = self.increments.to_le_bytes();
        let too_wide = |high: &[u8]| high.iter().any(|byte| *byte != 0);
        let out_of_range = || crate::num::overflow("time_increments");
        match self.len()? {
            3 => {
                if too_wide(&[b3, b4, b5, b6, b7]) {
                    return Err(out_of_range());
                }
                dst.put_slice(&[b0, b1, b2]);
            }
            4 => {
                if too_wide(&[b4, b5, b6, b7]) {
                    return Err(out_of_range());
                }
                dst.put_slice(&[b0, b1, b2, b3]);
            }
            5 => {
                if too_wide(&[b5, b6, b7]) {
                    return Err(out_of_range());
                }
                dst.put_slice(&[b0, b1, b2, b3, b4]);
            }
            other => {
                return Err(crate::Error::Protocol(
                    format!("timen: invalid encoded length {other}").into(),
                ))
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
/// A presentation of `datetime2` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. For dealing with
/// `datetime2`, use the `time` feature of this crate and its `PrimitiveDateTime`
/// type.
pub struct DateTime2 {
    date: Date,
    time: Time,
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl DateTime2 {
    /// Construct a new `DateTime2` from the date and time components.
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    /// The date component.
    pub fn date(self) -> Date {
        self.date
    }

    /// The time component.
    pub fn time(self) -> Time {
        self.time
    }

    pub(crate) async fn decode<R>(src: &mut R, n: usize, rlen: usize) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let time = Time::decode(src, n, rlen).await?;

        let mut bytes = [0u8; 4];
        src.read_exact(&mut bytes[..3]).await?;
        let date = Date::new(LittleEndian::read_u32(&bytes));

        Ok(Self::new(date, time))
    }
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Encode<BytesMut> for DateTime2 {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        self.time.encode(dst)?;

        let mut tmp = [0u8; 4];
        LittleEndian::write_u32(&mut tmp, self.date.days());
        assert_eq!(tmp[3], 0);
        dst.extend_from_slice(&tmp[0..3]);

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
/// A presentation of `datetimeoffset` type in the server.
///
/// # Warning
///
/// It isn't recommended to use this type directly. For dealing with
/// `datetimeoffset`, use the `time` feature of this crate and its `OffsetDateTime`
/// type with the correct timezone.
pub struct DateTimeOffset {
    datetime2: DateTime2,
    offset: i16,
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl DateTimeOffset {
    /// Construct a new `DateTimeOffset` from a `datetime2`, offset marking
    /// number of minutes from UTC.
    pub fn new(datetime2: DateTime2, offset: i16) -> Self {
        Self { datetime2, offset }
    }

    /// The date and time part.
    pub fn datetime2(self) -> DateTime2 {
        self.datetime2
    }

    /// Number of minutes from UTC.
    pub fn offset(self) -> i16 {
        self.offset
    }

    pub(crate) async fn decode<R>(src: &mut R, n: usize, rlen: u8) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let datetime2 = DateTime2::decode(src, n, usize::from(rlen)).await?;
        let offset = src.read_i16_le().await?;

        Ok(Self { datetime2, offset })
    }
}

#[cfg(feature = "tds73")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tds73")))]
impl Encode<BytesMut> for DateTimeOffset {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        self.datetime2.encode(dst)?;
        dst.put_i16_le(self.offset);

        Ok(())
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[cfg(feature = "tds73")]
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    #[cfg(feature = "tds73")]
    use bytes::BytesMut;

    #[cfg(feature = "tds73")]
    #[test]
    fn time_equality_is_exact_across_scales() {
        assert_eq!(Time::new(5, 1), Time::new(50, 2));
        assert_eq!(Time::new(0, 0), Time::new(0, 7));
        assert_ne!(Time::new(5, 1), Time::new(51, 2));
        // 2^53 + 1 increments is not representable as an f64; exact integer
        // comparison still tells them apart.
        assert_ne!(Time::new((1 << 53) + 1, 7), Time::new(1 << 53, 7));
        // An increment count that overflows at the common scale is unequal.
        assert_ne!(Time::new(u64::MAX, 0), Time::new(u64::MAX, 255));
    }

    #[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
    #[test]
    fn time_nanoseconds_scales_and_rejects_overflow() {
        assert_eq!(Time::new(1, 7).nanoseconds().unwrap(), 100);
        assert_eq!(Time::new(1, 9).nanoseconds().unwrap(), 1);
        assert_eq!(Time::new(1, 0).nanoseconds().unwrap(), 1_000_000_000);
        let overflow = crate::Error::Conversion("time_increments: arithmetic overflow".into());
        assert_eq!(Time::new(1, 10).nanoseconds().unwrap_err(), overflow);
        assert_eq!(Time::new(u64::MAX, 0).nanoseconds().unwrap_err(), overflow);
    }

    #[cfg(feature = "tds73")]
    #[test]
    fn time_encode_is_little_endian_and_rejects_values_wider_than_the_field() {
        let mut buf = BytesMut::new();
        Time::new(0x56_1234, 2).encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0x34, 0x12, 0x56]);

        let mut buf = BytesMut::new();
        Time::new(0xDEAD_BEEF, 4).encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0xEF, 0xBE, 0xAD, 0xDE]);

        let mut buf = BytesMut::new();
        Time::new(0x12_3456_789A, 7).encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0x9A, 0x78, 0x56, 0x34, 0x12]);

        let overflow = crate::Error::Conversion("time_increments: arithmetic overflow".into());
        assert_eq!(
            Time::new(1 << 24, 2)
                .encode(&mut BytesMut::new())
                .unwrap_err(),
            overflow
        );
        assert_eq!(
            Time::new(1 << 32, 4)
                .encode(&mut BytesMut::new())
                .unwrap_err(),
            overflow
        );
        assert_eq!(
            Time::new(1 << 40, 7)
                .encode(&mut BytesMut::new())
                .unwrap_err(),
            overflow
        );
    }

    #[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
    #[test]
    fn try_date_from_days_accepts_the_wire_range() {
        assert_eq!(try_date_from_days(0).unwrap().days(), 0);
        assert_eq!(
            try_date_from_days((1 << 24) - 1).unwrap().days(),
            (1 << 24) - 1
        );
    }

    #[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
    #[test]
    fn try_date_from_days_rejects_dates_outside_the_wire_range() {
        for days in [-1, i64::MIN, 1 << 24, i64::MAX] {
            let err = try_date_from_days(days).unwrap_err();
            assert!(
                matches!(&err, crate::Error::Conversion(m) if m.contains("0001-01-01..9999-12-31")),
                "{days}: {err:?}"
            );
        }
    }

    #[cfg(all(not(feature = "tds73"), any(feature = "jiff", feature = "time")))]
    #[test]
    fn try_legacy_days_enforces_the_datetime_range() {
        assert_eq!(try_legacy_days(-53_690).unwrap(), -53_690);
        assert_eq!(try_legacy_days(2_958_463).unwrap(), 2_958_463);
        assert!(try_legacy_days(-53_691).is_err());
        assert!(try_legacy_days(2_958_464).is_err());
        assert!(try_legacy_days(i64::MAX).is_err());
    }

    #[cfg(feature = "time")]
    #[test]
    fn nanos_of_day_is_total_over_the_clock_range() {
        assert_eq!(nanos_of_day(0, 0, 0, 0), 0);
        assert_eq!(
            nanos_of_day(23, 59, 59, 999_999_999),
            86_400 * 1_000_000_000 - 1
        );
    }

    #[cfg(all(feature = "tds73", any(feature = "jiff", feature = "time")))]
    #[test]
    fn offset_minutes_covers_the_full_offset_range() {
        assert_eq!(offset_minutes(0), 0);
        assert_eq!(offset_minutes(3600), 60);
        assert_eq!(offset_minutes(-93_599), -1559);
        assert_eq!(offset_minutes(93_599), 1559);
    }

    #[test]
    fn legacy_and_offset_accessors_return_constructor_values() {
        let datetime = DateTime::new(-1, 300);
        assert_eq!(datetime.days(), -1);
        assert_eq!(datetime.seconds_fragments(), 300);

        let small = SmallDateTime::new(2, 60);
        assert_eq!(small.days(), 2);
        assert_eq!(small.seconds_fragments(), 60);

        #[cfg(feature = "tds73")]
        {
            let dto = DateTimeOffset::new(DateTime2::new(Date::new(3), Time::new(4, 0)), -60);
            assert_eq!(dto.offset(), -60);
            assert_eq!(dto.datetime2().date().days(), 3);
        }
    }

    #[cfg(feature = "tds73")]
    #[tokio::test]
    async fn time_decode_covers_three_byte_shape_and_invalid_lengths() {
        let mut reader = BytesMut::from(&[0x34, 0x12, 0x56][..]).into_sql_read_bytes();
        let time = Time::decode(&mut reader, 2, 3).await.unwrap();
        assert_eq!(time.increments(), 0x56_1234);
        assert_eq!(time.scale(), 2);

        let mut reader = BytesMut::new().into_sql_read_bytes();
        assert!(matches!(
            Time::decode(&mut reader, 2, 4).await,
            Err(crate::Error::Protocol(_))
        ));
        assert!(matches!(
            Time::new(0, 8).len(),
            Err(crate::Error::Protocol(_))
        ));
    }
}
