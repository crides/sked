use chrono::{FixedOffset, Offset, Utc, Local};
use gluon::{vm::ExternModule, Thread};
use gluon_codegen::*;

#[derive(Clone, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.DateTime")]
#[gluon_trace(skip)]
pub struct DateTime(pub chrono::DateTime<FixedOffset>);

#[derive(Clone, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.Duration")]
#[gluon_trace(skip)]
pub struct Duration(pub chrono::Duration);

#[derive(Clone, Debug, Userdata, Trace, VmType)]
#[gluon_userdata(clone)]
#[gluon(vm_type = "time.TimeZone")]
#[gluon_trace(skip)]
pub struct TimeZone(pub chrono::FixedOffset);

fn format_datetime(time: &DateTime, format: &str) -> String {
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

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    thread.register_type::<DateTime>("time.DateTime", &[])?;
    thread.register_type::<Duration>("time.Duration", &[])?;
    thread.register_type::<TimeZone>("time.TimeZone", &[])?;
    ExternModule::new(
        thread,
        record! {
            type DateTime => DateTime,
            type Duration => Duration,
            format_datetime => primitive!(2, format_datetime),
            tz_east => primitive!(1, tz_east),
            tz_west => primitive!(1, tz_west),
            with_timezone => primitive!(2, with_timezone),
            to_local => primitive!(1, to_local),
            Utc => TimeZone(Utc.fix()),
            Local => TimeZone(*Local::now().offset()),
        },
    )
}
