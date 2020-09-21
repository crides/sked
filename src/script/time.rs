use std::fmt;

use chrono::{FixedOffset, Local, Offset, Timelike, Utc};
use gluon::{
    vm::{api::Getable, ExternModule, Result as GluonResult, Variants},
    Thread,
};
use gluon_codegen::*;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Clone, Copy, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.DateTime")]
#[gluon_trace(skip)]
pub struct DateTime(pub chrono::DateTime<FixedOffset>);

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

impl Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("$date", &self.0.to_rfc3339())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        enum Field {
            #[serde(rename = "$date")]
            Date,
        };

        struct DateTimeVisitor;

        impl<'de> Visitor<'de> for DateTimeVisitor {
            type Value = DateTime;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct DateTime")
            }

            fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<DateTime, V::Error> {
                let mut date = None;
                while let Some(Field::Date) = map.next_key()? {
                    if date.is_some() {
                        return Err(de::Error::duplicate_field("$date"));
                    }
                    let s: String = map.next_value()?;
                    date = Some(DateTime(
                        chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&s).unwrap(),
                    ))
                }
                date.ok_or_else(|| de::Error::missing_field("$date"))
            }
        }

        deserializer.deserialize_map(DateTimeVisitor)
    }
}

impl From<chrono::DateTime<Utc>> for DateTime {
    fn from(t: chrono::DateTime<Utc>) -> DateTime {
        DateTime(t.into())
    }
}

impl DateTime {
    pub fn to_utc(self) -> chrono::DateTime<Utc> {
        self.0.into()
    }

    fn format(&self, format: &str) -> String {
        self.0.format(format).to_string()
    }

    fn with_timezone(&self, tz: &TimeZone) -> DateTime {
        DateTime(self.0.with_timezone(&tz.0))
    }

    fn to_local(&self) -> DateTime {
        DateTime(self.0.with_timezone(Local::now().offset()))
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

#[derive(Clone, Copy, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.Time")]
#[gluon_trace(skip)]
pub struct Time(chrono::NaiveTime);

impl<'vm, 'value> Getable<'vm, 'value> for Time {
    type Proxy = Variants<'value>;
    fn to_proxy(_vm: &'vm Thread, value: Variants<'value>) -> GluonResult<Self::Proxy> {
        Ok(value)
    }
    fn from_proxy(vm: &'vm Thread, proxy: &'value mut Self::Proxy) -> Self {
        <Self as Getable<'vm, 'value>>::from_value(vm, proxy.clone())
    }
    fn from_value(vm: &'vm Thread, value: Variants<'value>) -> Self {
        *<&'value Time as Getable<'vm, 'value>>::from_value(vm, value)
    }
}

impl Time {
    pub fn to_secs(self) -> u32 {
        self.0.num_seconds_from_midnight()
    }

    pub fn from_secs(secs: u32) -> Self {
        Time(chrono::NaiveTime::from_num_seconds_from_midnight(secs, 0))
    }
}

#[derive(Clone, Copy, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.Duration")]
#[gluon_trace(skip)]
pub struct Duration(pub chrono::Duration);

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

#[derive(Clone, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.TimeZone")]
#[gluon_trace(skip)]
pub struct TimeZone(pub chrono::FixedOffset);

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
    thread.register_type::<Time>("time.Time", &[])?;
    thread.register_type::<Duration>("time.Duration", &[])?;
    thread.register_type::<TimeZone>("time.TimeZone", &[])?;
    ExternModule::new(
        thread,
        record! {
            timezone => record! {
                type TimeZone => TimeZone,
                Utc => TimeZone(Utc.fix()),
                Local => TimeZone(*Local::now().offset()),
                east => primitive!(1, TimeZone::east),
                west => primitive!(1, TimeZone::west),
            },
            time => record! {
                type DateTime => DateTime,
                format => primitive!(2, DateTime::format),
                with_timezone => primitive!(2, DateTime::with_timezone),
                to_local => primitive!(1, DateTime::to_local),
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
