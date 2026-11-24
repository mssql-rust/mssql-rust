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
    let nanos = secs
        .checked_mul(1_000_000_000)
        .ok_or_else(|| out_of_range("time of day"))?;
    from_nanos(nanos)
}

#[inline]
fn from_sec_fragments(sec_fragments: u64) -> crate::Result<Time> {
    // 1/300 s is 10^9 / 300 ns; multiply first, divide once (RCA-DIV-4).
    let nanos = sec_fragments
        .checked_mul(1_000_000_000)
        .and_then(|scaled| scaled.checked_div(300))
        .ok_or_else(|| out_of_range("time of day"))?;
    from_nanos(nanos)
}

#[inline]
fn to_days(date: Date, start_year: i32) -> i64 {
    (date - Date::from_calendar_date(start_year, Month::January, 1).unwrap()).whole_days()
}

#[inline]
#[cfg(not(feature = "tds73"))]
fn to_sec_fragments(from: Time) -> u32 {
    super::legacy_fragments(to_nanos(from))
}

#[cfg(feature = "tds73")]
from_sql!(
    PrimitiveDateTime:
        ColumnData::SmallDateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(i64::from(dt.days), 1900)?,
            from_secs(u64::from(dt.seconds_fragments).checked_mul(60).ok_or_else(|| out_of_range("time of day"))?)?,
        ))).transpose()?,
        ColumnData::DateTime2(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(i64::from(dt.date.days()), 1)?,
            from_nanos(dt.time.nanoseconds()?)?
        ))).transpose()?,
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(PrimitiveDateTime::new(
            from_days(i64::from(dt.days), 1900)?,
            from_sec_fragments(u64::from(dt.seconds_fragments))?
        ))).transpose()?;
    Time:
        ColumnData::Time(ref time) => time.map(|time| {
            from_nanos(time.nanoseconds()?)
        }).transpose()?;
    Date:
        ColumnData::Date(ref date) => date.map(|date| from_days(i64::from(date.days()), 1)).transpose()?;
    OffsetDateTime:
        ColumnData::DateTimeOffset(ref dto) => dto.map(|dto| {
            let date = from_days(i64::from(dto.datetime2.date.days()), 1)?;
            let dt = dto.datetime2;

            let time = from_nanos(dt.time.nanoseconds()?)?;

            let offset = UtcOffset::from_whole_seconds(
                i32::from(dto.offset)
                    .checked_mul(60)
                    .ok_or_else(|| out_of_range("datetimeoffset offset"))?,
            )
                .map_err(|_| out_of_range("datetimeoffset offset"))?;

            // The wire date/time is UTC. Shift to local wall-clock time with a
            // checked add, rather than `to_offset`, which panics when the
            // shifted value leaves the supported year range.
            let local = date
                .with_time(time)
                .checked_add(time::Duration::seconds(i64::from(offset.whole_seconds())))
                .ok_or_else(|| out_of_range("datetimeoffset"))?;

            Ok::<_, crate::Error>(local.assume_offset(offset))
        }).transpose()?
);

/// Nanoseconds since midnight of a time of day.
fn to_nanos(time: Time) -> u64 {
    super::nanos_of_day(time.hour(), time.minute(), time.second(), time.nanosecond())
}

#[cfg(feature = "tds73")]
fn time_to_wire(time: Time) -> super::Time {
    super::Time {
        increments: to_nanos(time) / 100,
        scale: 7,
    }
}

#[cfg(feature = "tds73")]
fn datetime_to_dt2(dt: PrimitiveDateTime) -> crate::Result<super::DateTime2> {
    let date = super::try_date_from_days(to_days(dt.date(), 1))?;
    Ok(super::DateTime2::new(date, time_to_wire(dt.time())))
}

#[cfg(feature = "tds73")]
fn offset_to_dto(odt: OffsetDateTime) -> crate::Result<super::DateTimeOffset> {
    let offset = super::offset_minutes(odt.offset().whole_seconds());
    let utc = odt.checked_to_offset(UtcOffset::UTC).ok_or_else(|| {
        crate::Error::Conversion(
            "date out of range for SQL Server datetimeoffset (0001-01-01..9999-12-31 in UTC)"
                .into(),
        )
    })?;
    Ok(super::DateTimeOffset::new(
        datetime_to_dt2(PrimitiveDateTime::new(utc.date(), utc.time()))?,
        offset,
    ))
}

// A value that cannot be represented (a date before 0001-01-01, or a UTC
// instant outside 0001-01-01..9999-12-31) becomes `ColumnData::Invalid`
// carrying the error, which every send path rejects with `Error::Conversion`
// before anything is written. `ToSql` cannot return an error, and a date is
// never clamped or substituted.
#[cfg(feature = "tds73")]
to_sql!(self_,
        Date: (super::fallible(ColumnData::Date), super::try_date_from_days(to_days(*self_, 1)));
        Time: (ColumnData::Time, time_to_wire(*self_));
        PrimitiveDateTime: (super::fallible(ColumnData::DateTime2), datetime_to_dt2(*self_));
        OffsetDateTime: (super::fallible(ColumnData::DateTimeOffset), offset_to_dto(*self_));
);

#[cfg(not(feature = "tds73"))]
fn datetime_to_legacy(dt: PrimitiveDateTime) -> crate::Result<super::DateTime> {
    let days = super::try_legacy_days(to_days(dt.date(), 1900))?;
    Ok(super::DateTime::new(days, to_sec_fragments(dt.time())))
}

#[cfg(not(feature = "tds73"))]
to_sql!(self_,
        PrimitiveDateTime: (super::fallible(ColumnData::DateTime), datetime_to_legacy(*self_));
);

#[cfg(not(feature = "tds73"))]
from_sql!(
    PrimitiveDateTime:
    ColumnData::DateTime(ref dt) => dt.map(|dt| {
        Ok::<_, crate::Error>(from_days(i64::from(dt.days), 1900)?.with_time(from_sec_fragments(u64::from(dt.seconds_fragments))?))
    }).transpose()?
);

#[cfg(all(test, feature = "tds73"))]
mod tests {
    use super::*;
    use crate::tds::time as tds;
    use crate::{FromSql, ToSql};

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

    fn invalid_message(data: &ColumnData<'_>) -> String {
        match data {
            ColumnData::Invalid(message) => message.to_string(),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    fn day(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).unwrap()
    }

    #[test]
    fn dates_before_year_one_become_invalid_not_a_panic() {
        let before = day(0, Month::December, 31);
        let midnight = Time::from_hms(0, 0, 0).unwrap();
        let pdt = PrimitiveDateTime::new(before, midnight);
        let odt = pdt.assume_utc();

        for data in [
            before.to_sql(),
            Some(before).to_sql(),
            pdt.to_sql(),
            odt.to_sql(),
            day(-4000, Month::January, 1).to_sql(),
        ] {
            assert!(
                invalid_message(&data).contains("0001-01-01..9999-12-31"),
                "{data:?}"
            );
        }
        assert!(matches!(Some(before).to_sql(), ColumnData::Invalid(_)));
    }

    #[test]
    fn year_one_and_year_9999_boundaries() {
        let first = day(1, Month::January, 1);
        let last = day(9999, Month::December, 31);
        assert_eq!(first.to_sql(), ColumnData::Date(Some(tds::Date::new(0))));
        assert_eq!(
            last.to_sql(),
            ColumnData::Date(Some(tds::Date::new(3_652_058)))
        );
        let midnight = Time::from_hms(0, 0, 0).unwrap();
        assert!(matches!(
            PrimitiveDateTime::new(first, midnight).to_sql(),
            ColumnData::DateTime2(Some(_))
        ));
        assert!(matches!(
            PrimitiveDateTime::new(last, Time::from_hms_nano(23, 59, 59, 999_999_900).unwrap())
                .to_sql(),
            ColumnData::DateTime2(Some(_))
        ));
        // In UTC the instant is in range, whatever the offset.
        assert!(matches!(
            PrimitiveDateTime::new(first, midnight)
                .assume_utc()
                .to_sql(),
            ColumnData::DateTimeOffset(Some(_))
        ));
        assert!(matches!(Some(first).to_sql(), ColumnData::Date(Some(_))));
    }

    #[test]
    fn offset_conversion_outside_the_supported_range_is_invalid() {
        // 9999-12-31T23:00 at -01:00 is 10000-01-01T00:00 in UTC.
        let odt = PrimitiveDateTime::new(
            day(9999, Month::December, 31),
            Time::from_hms(23, 0, 0).unwrap(),
        )
        .assume_offset(UtcOffset::from_hms(-1, 0, 0).unwrap());
        assert!(invalid_message(&odt.to_sql()).contains("datetimeoffset"));

        // 0001-01-01T00:30 at +01:00 is 0000-12-31T23:30 in UTC.
        let odt =
            PrimitiveDateTime::new(day(1, Month::January, 1), Time::from_hms(0, 30, 0).unwrap())
                .assume_offset(UtcOffset::from_hms(1, 0, 0).unwrap());
        assert!(matches!(odt.to_sql(), ColumnData::Invalid(_)));
    }

    #[test]
    fn valid_values_encode_to_the_same_wire_values() {
        let pdt = PrimitiveDateTime::new(
            day(2020, Month::April, 20),
            Time::from_hms_nano(16, 20, 5, 123_456_700).unwrap(),
        );
        let ColumnData::DateTime2(Some(dt2)) = pdt.to_sql() else {
            panic!("not datetime2")
        };
        assert_eq!(dt2.date().days(), 737_534);
        assert_eq!(dt2.time().increments(), 58_805 * 10_000_000 + 1_234_567);
        assert_eq!(dt2.time().scale(), 7);
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

    #[test]
    fn datetime_family_decodes_and_rejects_out_of_range_parts() {
        let first = day(1900, Month::January, 2);

        let small = ColumnData::SmallDateTime(Some(tds::SmallDateTime {
            days: 1,
            seconds_fragments: 90,
        }));
        assert_eq!(
            PrimitiveDateTime::from_sql(&small).unwrap().unwrap(),
            PrimitiveDateTime::new(first, Time::from_hms(1, 30, 0).unwrap())
        );

        let legacy = ColumnData::DateTime(Some(tds::DateTime::new(1, 300 * 61)));
        assert_eq!(
            PrimitiveDateTime::from_sql(&legacy).unwrap().unwrap(),
            PrimitiveDateTime::new(first, Time::from_hms(0, 1, 1).unwrap())
        );
        // A negative day count is a date before 1900.
        let before = ColumnData::DateTime(Some(tds::DateTime::new(-1, 0)));
        assert_eq!(
            PrimitiveDateTime::from_sql(&before)
                .unwrap()
                .unwrap()
                .date(),
            day(1899, Month::December, 31)
        );

        let dt2 = ColumnData::DateTime2(Some(tds::DateTime2::new(
            tds::Date::new(737_534),
            tds::Time {
                increments: 58_805 * 10_000_000 + 1_234_567,
                scale: 7,
            },
        )));
        assert_eq!(
            PrimitiveDateTime::from_sql(&dt2).unwrap().unwrap(),
            PrimitiveDateTime::new(
                day(2020, Month::April, 20),
                Time::from_hms_nano(16, 20, 5, 123_456_700).unwrap()
            )
        );

        // Time of day at or past 24h is rejected for every wire shape.
        let late_legacy = ColumnData::DateTime(Some(tds::DateTime::new(0, 300 * 86_400)));
        assert!(PrimitiveDateTime::from_sql(&late_legacy).is_err());
        let late_dt2 = ColumnData::DateTime2(Some(tds::DateTime2::new(
            tds::Date::new(0),
            tds::Time {
                increments: 864_000_000_000,
                scale: 7,
            },
        )));
        assert!(PrimitiveDateTime::from_sql(&late_dt2).is_err());
        // A day count past 9999-12-31 is rejected too.
        let far = ColumnData::DateTime(Some(tds::DateTime::new(i32::MAX, 0)));
        assert!(PrimitiveDateTime::from_sql(&far).is_err());
        let far_dt2 = ColumnData::DateTime2(Some(tds::DateTime2::new(
            tds::Date::new((1 << 24) - 1),
            tds::Time {
                increments: 0,
                scale: 7,
            },
        )));
        assert!(PrimitiveDateTime::from_sql(&far_dt2).is_err());
    }
}
