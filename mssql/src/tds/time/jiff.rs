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
use super::{DateTime2, DateTimeOffset, Time};
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
    // 1/300 s is 10^9 / 300 ns; multiply first, divide once (RCA-DIV-4).
    let nanos = sec_fragments
        .checked_mul(1_000_000_000)
        .and_then(|scaled| scaled.checked_div(300))
        .ok_or_else(|| out_of_range("time of day"))?;
    from_nanos(nanos)
}

#[inline]
#[cfg(feature = "tds73")]
fn from_mins(mins: u32) -> crate::Result<civil::Time> {
    let nanos = i64::from(mins)
        .checked_mul(60 * 1_000_000_000)
        .ok_or_else(|| out_of_range("time of day"))?;
    from_nanos(nanos)
}

#[inline]
fn to_days(date: civil::Date, start_year: i16) -> i64 {
    i64::from((date - civil::date(start_year, 1, 1)).get_days())
}

#[inline]
fn to_nanos(time: civil::Time) -> u64 {
    // jiff's hour, minute, second and sub-second nanosecond are never
    // negative (`unsigned_abs` is the identity there), and the total is
    // below 86 400 * 10^9, far inside u64 (RCA-EXC-2).
    u64::from(time.hour().unsigned_abs()) * 3_600_000_000_000
        + u64::from(time.minute().unsigned_abs()) * 60_000_000_000
        + u64::from(time.second().unsigned_abs()) * 1_000_000_000
        + u64::from(time.subsec_nanosecond().unsigned_abs())
}

#[inline]
#[cfg(not(feature = "tds73"))]
fn to_sec_fragments(time: civil::Time) -> u32 {
    super::legacy_fragments(to_nanos(time))
}

#[cfg(feature = "tds73")]
fn dt2_to_civil(dt2: &DateTime2) -> crate::Result<civil::DateTime> {
    let ns = i64::try_from(dt2.time.nanoseconds()?).map_err(|_| out_of_range("time of day"))?;
    Ok(civil::DateTime::from_parts(
        from_days(i64::from(dt2.date.days()), 1)?,
        from_nanos(ns)?,
    ))
}

#[cfg(feature = "tds73")]
fn civil_to_dt2(dt: civil::DateTime) -> crate::Result<DateTime2> {
    let date = super::try_date_from_days(to_days(dt.date(), 1))?;
    let time = Time {
        increments: to_nanos(dt.time()) / 100,
        scale: 7,
    };
    Ok(DateTime2::new(date, time))
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
fn timestamp_to_dt2(ts: &Timestamp) -> crate::Result<DateTime2> {
    civil_to_dt2(ts.to_zoned(jiff::tz::TimeZone::UTC).datetime())
}

#[cfg(feature = "tds73")]
fn zoned_to_dto(z: &Zoned) -> crate::Result<DateTimeOffset> {
    let offset = super::offset_minutes(z.offset().seconds());
    Ok(DateTimeOffset::new(
        timestamp_to_dt2(&z.timestamp())?,
        offset,
    ))
}

#[cfg(feature = "tds73")]
fn dto_to_zoned(dto: &DateTimeOffset) -> crate::Result<Zoned> {
    let ts = dto_to_timestamp(dto)?;
    let seconds = i32::from(dto.offset)
        .checked_mul(60)
        .ok_or_else(|| out_of_range("datetimeoffset offset"))?;
    let off = jiff::tz::Offset::from_seconds(seconds)
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
            from_days(i64::from(dt.days), 1900)?,
            from_mins(u32::from(dt.seconds_fragments))?,
        ))).transpose()?,
        ColumnData::DateTime2(ref dt) => dt.map(|dt| dt2_to_civil(&dt)).transpose()?,
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(civil::DateTime::from_parts(
            from_days(i64::from(dt.days), 1900)?,
            from_sec_fragments(i64::from(dt.seconds_fragments))?
        ))).transpose()?;
    civil::Time:
        ColumnData::Time(ref time) => time.map(|time| {
            from_nanos(i64::try_from(time.nanoseconds()?).map_err(|_| out_of_range("time of day"))?)
        }).transpose()?;
    civil::Date:
        ColumnData::Date(ref date) => date.map(|date| from_days(i64::from(date.days()), 1)).transpose()?
);

#[cfg(feature = "tds73")]
to_sql!(self_,
        civil::Date: (super::fallible(ColumnData::Date), super::try_date_from_days(to_days(*self_, 1)));
        civil::Time: (ColumnData::Time, Time { increments: to_nanos(*self_) / 100, scale: 7 });
        civil::DateTime: (super::fallible(ColumnData::DateTime2), civil_to_dt2(*self_));
        Timestamp: (super::fallible(ColumnData::DateTime2), timestamp_to_dt2(self_));
        Zoned: (super::fallible(ColumnData::DateTimeOffset), zoned_to_dto(self_));
);

#[cfg(feature = "tds73")]
into_sql!(self_,
        civil::Date: (super::fallible(ColumnData::Date), super::try_date_from_days(to_days(self_, 1)));
        civil::Time: (ColumnData::Time, Time { increments: to_nanos(self_) / 100, scale: 7 });
        civil::DateTime: (super::fallible(ColumnData::DateTime2), civil_to_dt2(self_));
        Timestamp: (super::fallible(ColumnData::DateTime2), timestamp_to_dt2(&self_));
        Zoned: (super::fallible(ColumnData::DateTimeOffset), zoned_to_dto(&self_));
);

#[cfg(not(feature = "tds73"))]
to_sql!(self_,
        civil::DateTime: (super::fallible(ColumnData::DateTime), {
            let days = super::try_legacy_days(to_days(self_.date(), 1900));
            days.map(|days| DateTime1::new(days, to_sec_fragments(self_.time())))
        });
);

#[cfg(not(feature = "tds73"))]
into_sql!(self_,
        civil::DateTime: (super::fallible(ColumnData::DateTime), {
            let days = super::try_legacy_days(to_days(self_.date(), 1900));
            days.map(|days| DateTime1::new(days, to_sec_fragments(self_.time())))
        });
);

#[cfg(not(feature = "tds73"))]
from_sql!(
    civil::DateTime:
        ColumnData::DateTime(ref dt) => dt.map(|dt| Ok::<_, crate::Error>(civil::DateTime::from_parts(
            from_days(i64::from(dt.days), 1900)?,
            from_sec_fragments(i64::from(dt.seconds_fragments))?
        ))).transpose()?
);

#[cfg(all(test, feature = "tds73"))]
mod tests {
    use super::*;
    use crate::tds::time::Date;
    use crate::{FromSql, FromSqlOwned, IntoSql, ToSql};
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

        let legacy = ColumnData::DateTime(Some(crate::tds::time::DateTime::new(1, 300)));
        assert_eq!(
            Some(civil::date(1900, 1, 2).at(0, 0, 1, 0)),
            civil::DateTime::from_sql(&legacy).unwrap()
        );
        let small = ColumnData::SmallDateTime(Some(crate::tds::time::SmallDateTime::new(1, 2)));
        assert_eq!(
            Some(civil::date(1900, 1, 2).at(0, 2, 0, 0)),
            civil::DateTime::from_sql(&small).unwrap()
        );
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

    fn assert_invalid(data: &ColumnData<'_>) {
        match data {
            ColumnData::Invalid(message) => {
                assert!(message.contains("0001-01-01..9999-12-31"), "{message}")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn dates_before_year_one_become_invalid_not_a_panic() {
        let before = civil::date(0, 12, 31);
        let dt = before.at(0, 0, 0, 0);
        let ts = TimeZone::UTC.to_timestamp(dt).unwrap();
        let zoned = ts.to_zoned(TimeZone::fixed(Offset::from_seconds(3600).unwrap()));

        assert_invalid(&before.to_sql());
        assert_invalid(&before.into_sql());
        assert_invalid(&Some(before).to_sql());
        assert_invalid(&Some(before).into_sql());
        assert_invalid(&dt.to_sql());
        assert_invalid(&dt.into_sql());
        assert_invalid(&ts.to_sql());
        assert_invalid(&ts.into_sql());
        assert_invalid(&zoned.to_sql());
        assert_invalid(&zoned.clone().into_sql());
        assert_invalid(&civil::date(-9999, 1, 1).to_sql());
    }

    #[test]
    fn year_one_and_year_9999_boundaries() {
        let first = civil::date(1, 1, 1);
        let last = civil::date(9999, 12, 31);
        assert_eq!(first.to_sql(), ColumnData::Date(Some(Date::new(0))));
        assert_eq!(last.to_sql(), ColumnData::Date(Some(Date::new(3_652_058))));
        assert_eq!(first.into_sql(), first.to_sql());
        assert!(matches!(
            first.at(0, 0, 0, 0).to_sql(),
            ColumnData::DateTime2(Some(_))
        ));
        assert!(matches!(
            last.at(23, 59, 59, 999_999_900).into_sql(),
            ColumnData::DateTime2(Some(_))
        ));
        let ts = TimeZone::UTC.to_timestamp(first.at(0, 0, 0, 0)).unwrap();
        assert!(matches!(ts.to_sql(), ColumnData::DateTime2(Some(_))));
        let zoned = ts.to_zoned(TimeZone::fixed(Offset::from_seconds(3600).unwrap()));
        assert!(matches!(
            zoned.to_sql(),
            ColumnData::DateTimeOffset(Some(_))
        ));
    }

    #[test]
    fn valid_values_encode_to_the_same_wire_values() {
        let dt = civil::date(2020, 4, 20).at(16, 20, 5, 123_456_700);
        let ColumnData::DateTime2(Some(dt2)) = dt.to_sql() else {
            panic!("not datetime2")
        };
        assert_eq!(dt2.date().days(), 737_534);
        assert_eq!(dt2.time().increments(), 58_805 * 10_000_000 + 1_234_567);
        assert_eq!(dt2.time().scale(), 7);
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

    #[test]
    fn owned_conversions_and_wrong_types_are_fallible() {
        let ts: Timestamp = "2020-04-20T16:20:00Z".parse().unwrap();
        assert_eq!(Timestamp::from_sql_owned(ts.into_sql()).unwrap(), Some(ts));

        let z = ts.to_zoned(TimeZone::fixed(Offset::from_seconds(3600).unwrap()));
        assert_eq!(
            Zoned::from_sql_owned(z.clone().into_sql())
                .unwrap()
                .unwrap()
                .timestamp(),
            z.timestamp()
        );

        let wrong = ColumnData::I32(Some(1));
        assert!(Timestamp::from_sql(&wrong).is_err());
        assert!(Timestamp::from_sql_owned(wrong.clone()).is_err());
        assert!(Zoned::from_sql(&wrong).is_err());
        assert!(Zoned::from_sql_owned(wrong.clone()).is_err());
        assert!(civil::DateTime::from_sql(&wrong).is_err());
        assert!(civil::Time::from_sql(&wrong).is_err());
        assert!(civil::Date::from_sql(&wrong).is_err());
    }

    #[test]
    fn conversion_helpers_reject_both_date_and_time_boundaries() {
        assert!(from_days(4_000_000, 1).is_err());
        assert!(from_nanos(-1).is_err());
        assert!(from_nanos(86_400 * 1_000_000_000).is_err());

        let invalid_offset = ColumnData::DateTimeOffset(Some(DateTimeOffset::new(
            civil_to_dt2(civil::date(2020, 1, 1).at(0, 0, 0, 0)).unwrap(),
            i16::MAX,
        )));
        assert!(Zoned::from_sql(&invalid_offset).is_err());
    }

    #[test]
    fn checked_conversion_helpers_reject_extreme_inputs() {
        assert!(from_days(i64::MAX, 1).is_err());
        assert!(from_nanos(-1).is_err());
        assert!(from_nanos(86_400 * 1_000_000_000).is_err());
        assert!(from_sec_fragments(i64::MAX).is_err());
        assert!(from_mins(u32::MAX).is_err());
        assert!(from_mins(24 * 60).is_err());
    }

    #[test]
    fn instants_past_the_timestamp_range_are_conversion_errors() {
        // 9999-12-31 is a valid datetime2 date but later than the largest
        // `Timestamp`.
        let last_day = || {
            DateTime2::new(
                Date::new(3_652_058),
                crate::tds::time::Time {
                    increments: 0,
                    scale: 7,
                },
            )
        };
        let dto = DateTimeOffset::new(last_day(), 0);
        assert!(dto_to_timestamp(&dto).is_err());
        assert!(dto_to_zoned(&dto).is_err());
        assert!(Timestamp::from_sql(&ColumnData::DateTimeOffset(Some(dto))).is_err());
        assert!(Timestamp::from_sql(&ColumnData::DateTime2(Some(last_day()))).is_err());
    }

    #[test]
    fn offsets_beyond_the_supported_range_are_conversion_errors() {
        let dt2 = DateTime2::new(
            Date::new(700_000),
            crate::tds::time::Time {
                increments: 0,
                scale: 7,
            },
        );
        // 40 hours is more than jiff's +/-25:59:59 offset limit.
        assert!(dto_to_zoned(&DateTimeOffset::new(dt2, 2_400)).is_err());
    }
}
