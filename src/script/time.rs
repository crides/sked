use chrono::{Datelike, FixedOffset, Local, Offset, TimeZone as _, Timelike, Utc};
use gluon::{
    vm::{api::Getable, ExternModule, Result as GluonResult, Variants},
    Thread,
};
use gluon_codegen::*;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::storage::time::{DateTime, Duration, TimeZone};

macro_rules! datetime_getter {
    ($($name:ident -> $typ:ty),+) => {
        $(fn $name(&self) -> $typ {
            self.0.$name()
        })+
    }
}

macro_rules! datetime_setter {
    ($($name:ident($($arg:ident: $arg_ty:ty),*)),+) => {
        $(fn $name(&self, $($arg: $arg_ty),*) -> Option<DateTime> {
            self.0.$name($($arg),*).map(|d| DateTime(d))
        })+
    }
}

impl<'vm, 'value> Getable<'vm, 'value> for DateTime {
    type Proxy = Variants<'value>;
    fn to_proxy(_vm: &'vm Thread, value: Variants<'value>) -> GluonResult<Self::Proxy> {
        Ok(value)
    }
    fn from_proxy(vm: &'vm Thread, proxy: &'value mut Self::Proxy) -> Self {
        <Self as Getable<'vm, 'value>>::from_value(vm, proxy.clone())
    }
    fn from_value(vm: &'vm Thread, value: Variants<'value>) -> Self {
        *<&'value DateTime as Getable<'vm, 'value>>::from_value(vm, value)
    }
}

impl DateTime {
    fn new(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<DateTime> {
        Utc.ymd_opt(y, m, d)
            .single()
            .map(|d| d.and_hms_opt(h, mi, s))
            .flatten()
            .map(|dt| DateTime(dt.into()))
    }

    fn new_local(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<DateTime> {
        Local
            .ymd_opt(y, m, d)
            .single()
            .map(|d| d.and_hms_opt(h, mi, s))
            .flatten()
            .map(|dt| DateTime(dt.into()))
    }

    fn from_timestamp(t: i64) -> DateTime {
        Utc.timestamp(t, 0).into()
    }

    datetime_getter!(year -> i32, month -> u32, day -> u32, hour -> u32, minute -> u32, second -> u32);

    datetime_setter!(
        with_year(y: i32),
        with_month(m: u32),
        with_day(d: u32),
        with_hour(h: u32),
        with_minute(m: u32),
        with_second(s: u32)
    );

    pub fn format(&self, format: &str) -> String {
        self.0.format(format).to_string()
    }

    fn with_timezone(&self, tz: &TimeZone) -> DateTime {
        DateTime(self.0.with_timezone(&tz.0))
    }

    fn to_local(&self) -> DateTime {
        DateTime(self.0.with_timezone(Local::now().offset()))
    }

    fn to_utc(&self) -> DateTime {
        DateTime(self.0.with_timezone(&FixedOffset::east(0)))
    }

    fn local_now(_: ()) -> DateTime {
        let now = Local::now();
        DateTime(now.with_timezone(now.offset()))
    }

    fn utc_now(_: ()) -> DateTime {
        Utc::now().into()
    }

    fn sub(&self, b: &DateTime) -> Duration {
        Duration(self.0 - b.0)
    }

    fn add(&self, b: &Duration) -> DateTime {
        DateTime(self.0 + b.0)
    }

    fn eq(&self, b: &DateTime) -> bool {
        self.0 == b.0
    }

    fn lt(&self, b: &DateTime) -> bool {
        self.0 < b.0
    }
}

impl<'vm, 'value> Getable<'vm, 'value> for Duration {
    type Proxy = Variants<'value>;
    fn to_proxy(_vm: &'vm Thread, value: Variants<'value>) -> GluonResult<Self::Proxy> {
        Ok(value)
    }
    fn from_proxy(vm: &'vm Thread, proxy: &'value mut Self::Proxy) -> Self {
        <Self as Getable<'vm, 'value>>::from_value(vm, proxy.clone())
    }
    fn from_value(vm: &'vm Thread, value: Variants<'value>) -> Self {
        *<&'value Duration as Getable<'vm, 'value>>::from_value(vm, value)
    }
}

impl Duration {
    fn millis(s: i64) -> Duration {
        Duration(chrono::Duration::milliseconds(s))
    }

    pub fn seconds(s: i64) -> Duration {
        Duration(chrono::Duration::seconds(s))
    }

    fn minutes(s: i64) -> Duration {
        Duration(chrono::Duration::minutes(s))
    }

    fn hours(s: i64) -> Duration {
        Duration(chrono::Duration::hours(s))
    }

    fn days(s: i64) -> Duration {
        Duration(chrono::Duration::days(s))
    }

    fn weeks(s: i64) -> Duration {
        Duration(chrono::Duration::weeks(s))
    }

    fn eq(&self, b: &Duration) -> bool {
        self.0 == b.0
    }

    fn lt(&self, b: &Duration) -> bool {
        self.0 < b.0
    }

    pub fn num_seconds(&self) -> i64 {
        self.0.num_seconds()
    }

    fn show(&self) -> String {
        self.format("%Dd %Hh %Mm %Ss")
    }

    fn format(&self, format: &str) -> String {
        let secs = self.num_seconds();
        let mins = secs / 60;
        let hours = secs / 3600;
        let days = secs / 86400;
        let weeks = secs / (86400 * 7);
        format
            .replace("%%", "%")
            .replace("%s", &secs.to_string())
            .replace("%m", &mins.to_string())
            .replace("%h", &hours.to_string())
            .replace("%d", &days.to_string())
            .replace("%w", &weeks.to_string())
            .replace("%S", &(secs % 60).to_string())
            .replace("%M", &(mins % 60).to_string())
            .replace("%H", &(hours % 24).to_string())
            .replace("%D", &(days % 7).to_string())
    }
}

impl TimeZone {
    fn east(secs: i32) -> TimeZone {
        TimeZone(FixedOffset::east(secs))
    }

    fn west(secs: i32) -> TimeZone {
        TimeZone(FixedOffset::west(secs))
    }
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    thread.register_type::<DateTime>("time.DateTime", &[])?;
    thread.register_type::<Duration>("time.Duration", &[])?;
    thread.register_type::<TimeZone>("time.TimeZone", &[])?;
    ExternModule::new(
        thread,
        record! {
            timezone => record! {
                type TimeZone => TimeZone,
                utc => TimeZone(Utc.fix()),
                local => TimeZone(*Local::now().offset()),
                east => primitive!(1, TimeZone::east),
                west => primitive!(1, TimeZone::west),
            },
            datetime => record! {
                type DateTime => DateTime,
                new => primitive!(6, DateTime::new),
                new_local => primitive!(6, DateTime::new_local),
                from_timestamp => primitive!(1, DateTime::from_timestamp),
                year => primitive!(1, DateTime::year),
                month => primitive!(1, DateTime::month),
                day => primitive!(1, DateTime::day),
                hour => primitive!(1, DateTime::hour),
                minute => primitive!(1, DateTime::minute),
                second => primitive!(1, DateTime::second),
                with_year => primitive!(2, DateTime::with_year),
                with_month => primitive!(2, DateTime::with_month),
                with_day => primitive!(2, DateTime::with_day),
                with_hour => primitive!(2, DateTime::with_hour),
                with_minute => primitive!(2, DateTime::with_minute),
                with_second => primitive!(2, DateTime::with_second),

                format => primitive!(2, DateTime::format),
                with_timezone => primitive!(2, DateTime::with_timezone),
                to_local => primitive!(1, DateTime::to_local),
                to_utc => primitive!(1, DateTime::to_utc),
                local_now => primitive!(1, DateTime::local_now),
                utc_now => primitive!(1, DateTime::utc_now),
                sub => primitive!(2, DateTime::sub),
                add => primitive!(2, DateTime::add),
                eq => primitive!(2, DateTime::eq),
                lt => primitive!(2, DateTime::lt),
            },
            duration => record! {
                type Duration => Duration,
                millis => primitive!(1, Duration::millis),
                seconds => primitive!(1, Duration::seconds),
                minutes => primitive!(1, Duration::minutes),
                hours => primitive!(1, Duration::hours),
                days => primitive!(1, Duration::days),
                weeks => primitive!(1, Duration::weeks),
                eq => primitive!(2, Duration::eq),
                lt => primitive!(2, Duration::lt),
                to_secs => primitive!(1, |d: Duration| d.num_seconds()),
                show => primitive!(1, Duration::show),
                format => primitive!(2, Duration::format),
            },
        },
    )
}
