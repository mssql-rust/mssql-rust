//! Mappings between TDS and and time crate types (with `time` feature flag
//! enabled).
//!
//! The time library offers better ergonomy and are highly recommended if
//! needing to modify and deal with date and time in SQL Server.

use std::time::Duration;
pub use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

use crate::tds::codec::ColumnData;

// `days` is signed because the legacy `DATETIME` type (unlike `SMALLDATETIME`
// or the TDS73 `DATE`) can carry a *negative* offset from 1900-01-01 for any
// date between SQL Server's minimum (1753-01-01) and 1899-12-31. Casting that
// to `u64` before multiplying (the old implementation) wraps to a huge value
// and overflows the multiply, panicking the client on an ordinary, in-range
// `datetime` value.
//
// The wire values below are server-controlled. A day count past what `time`
// can represent (a 3-byte `date` reaches well beyond 9999-12-31) used to panic
// in `Date + Duration`, and an offset beyond +-25:59:59 in
// `UtcOffset::from_whole_seconds(..).unwrap()`. A time of day at or past 24h
// (a 5-byte scale-7 `time` reaches ~30h, `smalldatetime` minutes reach 65535)
// silently wrapped around midnight. All are now conversion errors. Ported
// from upstream Tiberius (tiberius-rs/tiberius 0e977bf, 3e6afc3, 7803134).
fn out_of_range(what: &str) -> crate::Error {
    crate::Error::Conversion(format!("TDS {what} out of range for the time crate").into())
}

#[inline]
fn from_days(days: i64, start_year: i32) -> crate::Result<Date> {
    Date::from_calendar_date(start_year, Month::January, 1)
        .unwrap()
        .checked_add(time::Duration::days(days))
        .ok_or_else(|| out_of_range("date"))
}

#[inline]
fn from_nanos(nanos: u64) -> crate::Result<Time> {
    if nanos >= 86_400 * 1_000_000_000 {
        return Err(out_of_range("time of day"));
    }
    Ok(Time::from_hms(0, 0, 0).unwrap() + Duration::from_nanos(nanos))
}

#[inline]
#[cfg(feature = "tds73")]
fn from_secs(secs: u64) -> crate::Result<Time> {
    from_nanos(secs.saturating_mul(1_000_000_000))
}

#[inline]
fn from_sec_fragments(sec_fragments: u64) -> crate::Result<Time> {
    from_nanos(sec_fragments * (1e9 as u64) / 300)
}

#[inline]
fn to_days(date: Date, start_year: i32) -> i64 {
    (date - Date::from_calendar_date(start_year, Month::January, 1).unwrap()).whole_days()
}

#[inline]
#[cfg(not(feature = "tds73"))]
fn to_sec_fragments(from: Time) -> i64 {
    let nanos: i64 = (from - Time::from_hms(0, 0, 0).unwrap())
        .whole_nanoseconds()
        .try_into()
        .unwrap();

    nanos * 300 / (1e9 as i64)
}

#[cfg(feature = "tds73")]
from_sql!(
    PrimitiveDateTime:
        ColumnData::SmallDateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(dt.days as i64, 1900)?,
            from_secs(dt.seconds_fragments as u64 * 60)?,
        ))).transpose()?,
        ColumnData::DateTime2(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(dt.date.days() as i64, 1)?,
            from_nanos(dt.time.increments * 10u64.pow(9 - dt.time.scale as u32))?
        ))).transpose()?,
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(dt.days as i64, 1900)?,
            from_sec_fragments(dt.seconds_fragments as u64)?
        ))).transpose()?;
    Time:
        ColumnData::Time(ref time) => time.map(|time| {
            from_nanos(time.increments * 10u64.pow(9 - time.scale as u32))
        }).transpose()?;
    Date:
        ColumnData::Date(ref date) => date.map(|date| from_days(date.days() as i64, 1)).transpose()?;
    OffsetDateTime:
        ColumnData::DateTimeOffset(ref dto) => dto.map(|dto| {
            let date = from_days(dto.datetime2.date.days() as i64, 1)?;
            let dt = dto.datetime2;

            let time = from_nanos(dt.time.increments * 10u64.pow(9 - dt.time.scale as u32))?;

            let offset = UtcOffset::from_whole_seconds(dto.offset as i32 * 60)
                .map_err(|_| out_of_range("datetimeoffset offset"))?;

            // The wire date/time is UTC. Shift to local wall-clock time with a
            // checked add, rather than `to_offset`, which panics when the
            // shifted value leaves the supported year range.
            let local = date
                .with_time(time)
                .checked_add(time::Duration::seconds(offset.whole_seconds() as i64))
                .ok_or_else(|| out_of_range("datetimeoffset"))?;

            Ok::<_, crate::Error>(local.assume_offset(offset))
        }).transpose()?
);

#[cfg(feature = "tds73")]
to_sql!(self_,
        Date: (ColumnData::Date, super::Date::new(to_days(*self_, 1) as u32));
        Time: (ColumnData::Time, {
            let nanos: u64 = (*self_ - Time::from_hms(0, 0, 0).unwrap()).whole_nanoseconds().try_into().unwrap();
            let increments = nanos / 100;

            super::Time {increments, scale: 7}
        });
        PrimitiveDateTime: (ColumnData::DateTime2, {
            let time = self_.time();
            let nanos: u64 = (time - Time::from_hms(0, 0, 0).unwrap()).whole_nanoseconds().try_into().unwrap();
            let increments = nanos / 100;

            let date = super::Date::new(to_days(self_.date(), 1) as u32);
            let time = super::Time {increments, scale: 7};

            super::DateTime2::new(date, time)
        });
        OffsetDateTime: (ColumnData::DateTimeOffset, {
            let tz = self_.offset();
            let offset = (tz.whole_seconds() / 60) as i16;

            let utc_date = self_.to_offset(UtcOffset::UTC);

            let nanos: u64 = (utc_date.time() - Time::from_hms(0, 0, 0).unwrap()).whole_nanoseconds().try_into().unwrap();

            let date = super::Date::new(to_days(utc_date.date(), 1) as u32);
            let time = super::Time { increments: nanos / 100, scale: 7 };

            super::DateTimeOffset::new(super::DateTime2::new(date, time), offset)
        });
);

#[cfg(not(feature = "tds73"))]
to_sql!(self_,
        PrimitiveDateTime: (ColumnData::DateTime, {
            let date = self_.date();
            let time = self_.time();

            let days = to_days(date, 1900) as i32;
            let seconds_fragments = to_sec_fragments(time);

            super::DateTime::new(days, seconds_fragments as u32)
        });
);

#[cfg(not(feature = "tds73"))]
from_sql!(
    PrimitiveDateTime:
    ColumnData::DateTime(ref dt) => dt.map(|dt| {
        Ok::<_, crate::Error>(from_days(dt.days as i64, 1900)?.with_time(from_sec_fragments(dt.seconds_fragments as u64)?))
    }).transpose()?
);

#[cfg(all(test, feature = "tds73"))]
mod tests {
    use super::*;
    use crate::tds::time as tds;
    use crate::FromSql;

    // Hostile/out-of-range wire values must be conversion errors, not panics
    // or silent wrap-around.
    #[test]
    fn out_of_range_wire_values_error_instead_of_panicking() {
        let midnight = tds::Time {
            increments: 0,
            scale: 7,
        };

        // 3-byte date day count past 9999-12-31.
        let d = ColumnData::Date(Some(tds::Date::new((1 << 24) - 1)));
        assert!(Date::from_sql(&d).is_err());

        // 5-byte scale-7 time of ~30.5h used to wrap past midnight.
        let t = ColumnData::Time(Some(tds::Time {
            increments: (1u64 << 40) - 1,
            scale: 7,
        }));
        assert!(Time::from_sql(&t).is_err());

        // smalldatetime minutes >= 1440.
        let sdt = ColumnData::SmallDateTime(Some(tds::SmallDateTime {
            days: 0,
            seconds_fragments: u16::MAX,
        }));
        assert!(PrimitiveDateTime::from_sql(&sdt).is_err());

        // datetimeoffset offset beyond what UtcOffset can hold.
        let dto = ColumnData::DateTimeOffset(Some(tds::DateTimeOffset::new(
            tds::DateTime2::new(tds::Date::new(0), midnight),
            i16::MAX,
        )));
        assert!(OffsetDateTime::from_sql(&dto).is_err());

        // 9999-12-31 UTC shifted to +14:00 leaves the supported range.
        let last_day = to_days(
            Date::from_calendar_date(9999, Month::December, 31).unwrap(),
            1,
        );
        let dto = ColumnData::DateTimeOffset(Some(tds::DateTimeOffset::new(
            tds::DateTime2::new(
                tds::Date::new(last_day as u32),
                tds::Time {
                    increments: 23 * 36_000_000_000,
                    scale: 7,
                },
            ),
            14 * 60,
        )));
        assert!(OffsetDateTime::from_sql(&dto).is_err());
    }

    #[test]
    fn datetimeoffset_keeps_offset_and_instant() {
        let dto = ColumnData::DateTimeOffset(Some(tds::DateTimeOffset::new(
            tds::DateTime2::new(
                tds::Date::new(737_000),
                tds::Time {
                    increments: 10 * 36_000_000_000, // 10:00 UTC
                    scale: 7,
                },
            ),
            -5 * 60,
        )));
        let odt = OffsetDateTime::from_sql(&dto).unwrap().unwrap();
        assert_eq!(odt.offset().whole_hours(), -5);
        assert_eq!(odt.hour(), 5);
        assert_eq!(odt.to_offset(UtcOffset::UTC).hour(), 10);
    }
}
