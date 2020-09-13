use chrono::{FixedOffset, Local, Offset, Utc};
use gluon::{
    vm::{api::Getable, ExternModule, Result as GluonResult, Variants},
    Thread,
};
use gluon_codegen::*;

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

#[derive(Clone, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.TimeZone")]
#[gluon_trace(skip)]
pub struct TimeZone(pub chrono::FixedOffset);

fn format(time: &DateTime, format: &str) -> String {
    format!("{}", time.0.format(format))
}

fn with_timezone(time: &DateTime, tz: &TimeZone) -> DateTime {
    DateTime(time.0.with_timezone(&tz.0))
}

fn tz_east(secs: i32) -> TimeZone {
    TimeZone(FixedOffset::east(secs))
}

fn tz_west(secs: i32) -> TimeZone {
    TimeZone(FixedOffset::west(secs))
}

fn to_local(time: &DateTime) -> DateTime {
    DateTime(time.0.with_timezone(Local::now().offset()))
}

fn local_now(_: ()) -> DateTime {
    let now = Local::now();
    DateTime(now.with_timezone(now.offset()))
}

fn utc_now(_: ()) -> DateTime {
    DateTime(Utc::now().into())
}

fn time_sub(a: &DateTime, b: &DateTime) -> Duration {
    Duration(a.0 - b.0)
}

fn time_add(a: &DateTime, b: &Duration) -> DateTime {
    DateTime(a.0 + b.0)
}

fn time_eq(a: &DateTime, b: &DateTime) -> bool {
    a.0 == b.0
}

fn time_lt(a: &DateTime, b: &DateTime) -> bool {
    a.0 < b.0
}

fn duration_millis(s: i64) -> Duration {
    Duration(chrono::Duration::milliseconds(s))
}

fn duration_seconds(s: i64) -> Duration {
    Duration(chrono::Duration::seconds(s))
}

fn duration_minutes(s: i64) -> Duration {
    Duration(chrono::Duration::minutes(s))
}

fn duration_hours(s: i64) -> Duration {
    Duration(chrono::Duration::hours(s))
}

fn duration_days(s: i64) -> Duration {
    Duration(chrono::Duration::days(s))
}

fn duration_weeks(s: i64) -> Duration {
    Duration(chrono::Duration::weeks(s))
}

fn duration_eq(a: &Duration, b: &Duration) -> bool {
    a.0 == b.0
}

fn duration_lt(a: &Duration, b: &Duration) -> bool {
    a.0 < b.0
}

fn show_duration(d: &Duration) -> String {
    let d = d.0;
    let secs = d.num_seconds();
    let nano_secs = (d - chrono::Duration::seconds(secs)).num_nanoseconds().unwrap();
    format!("Duration({}.{:09})", secs, nano_secs)
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
                Utc => TimeZone(Utc.fix()),
                Local => TimeZone(*Local::now().offset()),
                east => primitive!(1, tz_east),
                west => primitive!(1, tz_west),
            },
            time => record! {
                type DateTime => DateTime,
                format => primitive!(2, format),
                with_timezone => primitive!(2, with_timezone),
                to_local => primitive!(1, to_local),
                local_now => primitive!(1, local_now),
                utc_now => primitive!(1, utc_now),
                sub => primitive!(2, time_sub),
                add => primitive!(2, time_add),
                eq => primitive!(2, time_eq),
                lt => primitive!(2, time_lt),
            },
            duration => record! {
                type Duration => Duration,
                millis => primitive!(1, duration_millis),
                seconds => primitive!(1, duration_seconds),
                minutes => primitive!(1, duration_minutes),
                hours => primitive!(1, duration_hours),
                days => primitive!(1, duration_days),
                weeks => primitive!(1, duration_weeks),
                eq => primitive!(2, duration_eq),
                lt => primitive!(2, duration_lt),
                show => primitive!(1, show_duration),
            },
        },
    )
}
