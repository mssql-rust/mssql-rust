//! Mappings between TDS and `jiff` types (with `jiff` feature flag enabled).
//!
//! `jiff` is the recommended date/time library for new projects. The mapping
//! is:
//!
//! | SQL Server type | `jiff` type |
//! |---|---|
//! | `date` | [`civil::Date`](jiff::civil::Date) |
//! | `time` | [`civil::Time`](jiff::civil::Time) |
//! | `datetime`, `datetime2`, `smalldatetime` | [`civil::DateTime`](jiff::civil::DateTime) |
//! | `datetimeoffset` | [`Timestamp`](jiff::Timestamp) (instant, UTC) or [`Zoned`](jiff::Zoned) (fixed offset kept) |
//!
//! Writing a `Timestamp` sends a `datetime2` in UTC; writing a `Zoned` sends a
//! `datetimeoffset` carrying the zone's UTC offset at that instant.

#[cfg(not(feature = "tds73"))]
use super::DateTime as DateTime1;
#[cfg(feature = "tds73")]
use super::{Date, DateTime2, DateTimeOffset, Time};
use crate::tds::codec::ColumnData;
pub use jiff::civil::{Date as CivilDate, DateTime as CivilDateTime, Time as CivilTime};
#[cfg(feature = "tds73")]
pub use jiff::{Timestamp, Zoned};

use jiff::civil;
use jiff::Span;

// The wire values below are server-controlled. A day count past what jiff
// can represent (e.g. a 3-byte `date` beyond 9999-12-31), or a time of day
// at or past 24h (a 5-byte scale-7 `time` reaches ~30h, `smalldatetime`
// minutes reach 65535, `datetime` fragments ~165 days), used to panic via
// `Span::days`/`.expect`. They are now conversion errors, in the spirit of
// upstream Tiberius's equivalent chrono/time fixes (tiberius-rs/tiberius
// 3e6afc3, 0e977bf, 7803134).
fn out_of_range(what: &str) -> crate::Error {
    crate::Error::Conversion(format!("TDS {what} out of range for jiff").into())
}

#[inline]
fn from_days(days: i64, start_year: i16) -> crate::Result<civil::Date> {
    let span = Span::new()
        .try_days(days)
        .map_err(|_| out_of_range("date"))?;
    civil::date(start_year, 1, 1)
        .checked_add(span)
        .map_err(|_| out_of_range("date"))
}

#[inline]
fn from_nanos(nanos: i64) -> crate::Result<civil::Time> {
    if !(0..86_400 * 1_000_000_000).contains(&nanos) {
        return Err(out_of_range("time of day"));
    }
    civil::Time::midnight()
        .checked_add(Span::new().nanoseconds(nanos))
        .map_err(|_| out_of_range("time of day"))
}

#[inline]
fn from_sec_fragments(sec_fragments: i64) -> crate::Result<civil::Time> {
    from_nanos(sec_fragments * (1e9 as i64) / 300)
}

#[inline]
#[cfg(feature = "tds73")]
fn from_mins(mins: u32) -> crate::Result<civil::Time> {
    from_nanos(mins as i64 * 60 * 1_000_000_000)
}

#[inline]
fn to_days(date: civil::Date, start_year: i16) -> i64 {
    (date - civil::date(start_year, 1, 1)).get_days() as i64
}

#[inline]
fn to_nanos(time: civil::Time) -> u64 {
    time.hour() as u64 * 3_600_000_000_000
        + time.minute() as u64 * 60_000_000_000
        + time.second() as u64 * 1_000_000_000
        + time.subsec_nanosecond() as u64
}

#[inline]
#[cfg(not(feature = "tds73"))]
fn to_sec_fragments(time: civil::Time) -> i64 {
    to_nanos(time) as i64 * 300 / (1e9 as i64)
}

#[cfg(feature = "tds73")]
fn dt2_to_civil(dt2: &DateTime2) -> crate::Result<civil::DateTime> {
    let ns = dt2.time.increments as i64 * 10i64.pow(9 - dt2.time.scale as u32);
    Ok(civil::DateTime::from_parts(
        from_days(dt2.date.days() as i64, 1)?,
        from_nanos(ns)?,
    ))
}

#[cfg(feature = "tds73")]
fn civil_to_dt2(dt: civil::DateTime) -> DateTime2 {
    let date = Date::new(to_days(dt.date(), 1) as u32);
    let time = Time {
        increments: to_nanos(dt.time()) / 100,
        scale: 7,
    };
    DateTime2::new(date, time)
}

#[cfg(feature = "tds73")]
fn dto_to_timestamp(dto: &DateTimeOffset) -> crate::Result<Timestamp> {
    // On the wire the date and time parts of a `datetimeoffset` are UTC; the
    // offset only records the display zone.
    jiff::tz::TimeZone::UTC
        .to_timestamp(dt2_to_civil(&dto.datetime2)?)
        .map_err(|e| crate::Error::Conversion(e.to_string().into()))
}

#[cfg(feature = "tds73")]
fn timestamp_to_dt2(ts: &Timestamp) -> DateTime2 {
    civil_to_dt2(ts.to_zoned(jiff::tz::TimeZone::UTC).datetime())
}

#[cfg(feature = "tds73")]
fn zoned_to_dto(z: &Zoned) -> DateTimeOffset {
    let offset = (z.offset().seconds() / 60) as i16;
    DateTimeOffset::new(timestamp_to_dt2(&z.timestamp()), offset)
}

#[cfg(feature = "tds73")]
fn dto_to_zoned(dto: &DateTimeOffset) -> crate::Result<Zoned> {
    let ts = dto_to_timestamp(dto)?;
    let off = jiff::tz::Offset::from_seconds(dto.offset as i32 * 60)
        .map_err(|e| crate::Error::Conversion(e.to_string().into()))?;
    Ok(ts.to_zoned(jiff::tz::TimeZone::fixed(off)))
}

#[cfg(feature = "tds73")]
impl<'a> crate::FromSql<'a> for Timestamp {
    fn from_sql(data: &'a ColumnData<'static>) -> crate::Result<Option<Self>> {
        match data {
            ColumnData::DateTimeOffset(dto) => dto.as_ref().map(dto_to_timestamp).transpose(),
            ColumnData::DateTime2(dt2) => dt2
                .as_ref()
                .map(|dt2| {
                    jiff::tz::TimeZone::UTC
                        .to_timestamp(dt2_to_civil(dt2)?)
                        .map_err(|e| crate::Error::Conversion(e.to_string().into()))
                })
                .transpose(),
            _ => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a Timestamp value", data).into(),
            )),
        }
    }
}

#[cfg(feature = "tds73")]
impl crate::FromSqlOwned for Timestamp {
    fn from_sql_owned(data: ColumnData<'static>) -> crate::Result<Option<Self>> {
        crate::FromSql::from_sql(&data)
    }
}

#[cfg(feature = "tds73")]
impl<'a> crate::FromSql<'a> for Zoned {
    fn from_sql(data: &'a ColumnData<'static>) -> crate::Result<Option<Self>> {
        match data {
            ColumnData::DateTimeOffset(dto) => dto.as_ref().map(dto_to_zoned).transpose(),
            _ => Err(crate::Error::Conversion(
                format!("cannot interpret {:?} as a Zoned value", data).into(),
            )),
        }
    }
}

#[cfg(feature = "tds73")]
impl crate::FromSqlOwned for Zoned {
    fn from_sql_owned(data: ColumnData<'static>) -> crate::Result<Option<Self>> {
        crate::FromSql::from_sql(&data)
    }
}

#[cfg(feature = "tds73")]
from_sql!(
    civil::DateTime:
        ColumnData::SmallDateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(civil::DateTime::from_parts(
            from_days(dt.days as i64, 1900)?,
            from_mins(dt.seconds_fragments as u32)?,
        ))).transpose()?,
        ColumnData::DateTime2(ref dt) => dt.map(|dt| dt2_to_civil(&dt)).transpose()?,
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(civil::DateTime::from_parts(
            from_days(dt.days as i64, 1900)?,
            from_sec_fragments(dt.seconds_fragments as i64)?
        ))).transpose()?;
    civil::Time:
        ColumnData::Time(ref time) => time.map(|time| {
            from_nanos(time.increments as i64 * 10i64.pow(9 - time.scale as u32))
        }).transpose()?;
    civil::Date:
        ColumnData::Date(ref date) => date.map(|date| from_days(date.days() as i64, 1)).transpose()?
);

#[cfg(feature = "tds73")]
to_sql!(self_,
        civil::Date: (ColumnData::Date, Date::new(to_days(*self_, 1) as u32));
        civil::Time: (ColumnData::Time, Time { increments: to_nanos(*self_) / 100, scale: 7 });
        civil::DateTime: (ColumnData::DateTime2, civil_to_dt2(*self_));
        Timestamp: (ColumnData::DateTime2, timestamp_to_dt2(self_));
        Zoned: (ColumnData::DateTimeOffset, zoned_to_dto(self_));
);

#[cfg(feature = "tds73")]
into_sql!(self_,
        civil::Date: (ColumnData::Date, Date::new(to_days(self_, 1) as u32));
        civil::Time: (ColumnData::Time, Time { increments: to_nanos(self_) / 100, scale: 7 });
        civil::DateTime: (ColumnData::DateTime2, civil_to_dt2(self_));
        Timestamp: (ColumnData::DateTime2, timestamp_to_dt2(&self_));
        Zoned: (ColumnData::DateTimeOffset, zoned_to_dto(&self_));
);

#[cfg(not(feature = "tds73"))]
to_sql!(self_,
        civil::DateTime: (ColumnData::DateTime, {
            let days = to_days(self_.date(), 1900) as i32;
            DateTime1::new(days, to_sec_fragments(self_.time()) as u32)
        });
);

#[cfg(not(feature = "tds73"))]
into_sql!(self_,
        civil::DateTime: (ColumnData::DateTime, {
            let days = to_days(self_.date(), 1900) as i32;
            DateTime1::new(days, to_sec_fragments(self_.time()) as u32)
        });
);

#[cfg(not(feature = "tds73"))]
from_sql!(
    civil::DateTime:
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(civil::DateTime::from_parts(
            from_days(dt.days as i64, 1900)?,
            from_sec_fragments(dt.seconds_fragments as i64)?
        ))).transpose()?
);

#[cfg(all(test, feature = "tds73"))]
mod tests {
    use super::*;
    use crate::{FromSql, IntoSql};
    use jiff::tz::{Offset, TimeZone};

    #[test]
    fn civil_roundtrip() {
        let dt = civil::date(2020, 4, 20).at(16, 20, 5, 123_456_700);
        let cd = dt.into_sql();
        assert_eq!(Some(dt), civil::DateTime::from_sql(&cd).unwrap());
        let d = civil::date(1, 1, 1);
        assert_eq!(Some(d), civil::Date::from_sql(&d.into_sql()).unwrap());
        let t = civil::time(23, 59, 59, 0);
        assert_eq!(Some(t), civil::Time::from_sql(&t.into_sql()).unwrap());
    }

    // Hostile/out-of-range wire values must be conversion errors, not panics.
    #[test]
    fn out_of_range_wire_values_error_instead_of_panicking() {
        use crate::tds::time::{SmallDateTime, Time};

        // 5-byte scale-7 time: 2^40 - 1 increments of 100ns is ~30.5h.
        let t = ColumnData::Time(Some(Time {
            increments: (1u64 << 40) - 1,
            scale: 7,
        }));
        assert!(civil::Time::from_sql(&t).is_err());

        // smalldatetime minutes >= 1440.
        let sdt = ColumnData::SmallDateTime(Some(SmallDateTime {
            days: 0,
            seconds_fragments: u16::MAX,
        }));
        assert!(civil::DateTime::from_sql(&sdt).is_err());

        // 3-byte date day count past 9999-12-31.
        let d = ColumnData::Date(Some(Date::new((1 << 24) - 1)));
        assert!(civil::Date::from_sql(&d).is_err());

        // ...and the same via datetime2/Timestamp.
        let dt2 = ColumnData::DateTime2(Some(DateTime2::new(
            Date::new((1 << 24) - 1),
            Time {
                increments: 0,
                scale: 7,
            },
        )));
        assert!(civil::DateTime::from_sql(&dt2).is_err());
        assert!(Timestamp::from_sql(&dt2).is_err());
    }

    #[test]
    fn timestamp_and_zoned_roundtrip() {
        let ts: Timestamp = "2020-04-20T16:20:00Z".parse().unwrap();
        assert_eq!(Some(ts), Timestamp::from_sql(&ts.into_sql()).unwrap());

        let z = ts.to_zoned(TimeZone::fixed(Offset::from_seconds(3 * 3600).unwrap()));
        let cd = z.clone().into_sql();
        let back = Zoned::from_sql(&cd).unwrap().unwrap();
        assert_eq!(z.timestamp(), back.timestamp());
        assert_eq!(z.offset(), back.offset());
        assert_eq!(Some(ts), Timestamp::from_sql(&cd).unwrap());
    }
}
