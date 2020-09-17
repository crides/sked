use std::iter::once;

use bson::{doc, Document, Bson};

use crate::script::{
    time::{DateTime, Duration, Time},
    sched::{Object, STORE},
};
use crate::storage::Result as StorageResult;
use crate::util::bits;

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub struct Task {
    pub object: Object,
    pub start: DateTime,
    pub deadline: Time,
    pub every: Every,
    pub stop: Stop,
}

impl Task {
    pub fn new(name: &str, start: DateTime, every: Every, stop: Stop, deadline: Time) -> StorageResult<i32> {
        STORE.lock().unwrap().create_task(name, start, every, stop, deadline)
    }

    pub fn get(id: i32) -> StorageResult<Task> {
        STORE.lock().unwrap().get_task(id)
    }
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub struct Event {
    pub object: Object,
    pub start: DateTime,
    pub event_start: Time,
    pub duration: Duration,
    pub every: Every,
    pub stop: Stop,
}

impl Event {
    pub fn new(name: &str, start: DateTime, every: Every, stop: Stop, event_start: Time, duration: Duration) -> StorageResult<i32> {
        STORE.lock().unwrap().create_event(name, start, every, stop, event_start, duration)
    }

    pub fn get(id: i32) -> StorageResult<Event> {
        STORE.lock().unwrap().get_event(id)
    }
}

#[derive(Clone, Copy, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum Weekday {
    Mon = 1,
    Tue = 2,
    Wed = 3,
    Thu = 4,
    Fri = 5,
    Sat = 6,
    Sun = 7,
}

impl Weekday {
    pub fn from_num(i: usize) -> Weekday {
        match i {
            1 => Weekday::Mon,
            2 => Weekday::Tue,
            3 => Weekday::Wed,
            4 => Weekday::Thu,
            5 => Weekday::Fri,
            6 => Weekday::Sat,
            7 => Weekday::Sun,
            _ => panic!("too large"),
        }
    }
}

#[derive(Clone, Copy, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum Month {
    Jan = 1,
    Feb = 2,
    Mar = 3,
    Apr = 4,
    May = 5,
    Jun = 6,
    Jul = 7,
    Aug = 8,
    Sep = 9,
    Oct = 10,
    Nov = 11,
    Dec = 12,
}

impl Month {
    pub fn from_num(i: usize) -> Month {
        match i {
            1 => Month::Jan,
            2 => Month::Feb,
            3 => Month::Mar,
            4 => Month::Apr,
            5 => Month::May,
            6 => Month::Jun,
            7 => Month::Jul,
            8 => Month::Aug,
            9 => Month::Sep,
            10 => Month::Oct,
            11 => Month::Nov,
            12 => Month::Dec,
            _ => panic!("too large"),
        }
    }
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum Every {
    Day,
    Week(Vec<Weekday>),
    Month(Vec<u32>),
    Year(Vec<(Month, Vec<u32>)>),
}

impl Every {
    pub fn to_doc(self) -> Document {
        match self {
            Every::Day => doc! { "head": "day" },
            Every::Week(v) => {
                let mut bit_arr = 0i32;
                for d in v {
                    bit_arr |= 1 << d as u8;
                }
                doc! { "head": "week", "data": bit_arr }
            }
            Every::Month(v) => {
                let mut bit_arr = 0i32;
                for d in v {
                    bit_arr |= 1 << d;
                }
                doc! { "head": "month", "data": bit_arr }
            }
            Every::Year(v) => {
                let res: Vec<i32> = v
                    .into_iter()
                    .flat_map(|(m, ds)| {
                        let mut bit_arr = 0i32;
                        for d in ds {
                            bit_arr |= 1 << d;
                        }
                        once(m as i32).chain(once(bit_arr))
                    })
                    .collect();
                doc! { "head": "year", "data": res }
            }
        }
    }

    pub fn from_doc(mut d: Document) -> Every {
        match d.get_str("head") {
            Ok("day") => Every::Day,
            Ok("week") => {
                let bits = bits(d.get_i32("data").unwrap() as u32, 7);
                let days = bits.enumerate().filter(|(_, b)| *b).map(|(i, _)| Weekday::from_num(i)).collect();
                Every::Week(days)
            },
            Ok("month") => {
                let bits = bits(d.get_i32("data").unwrap() as u32, 31);
                let days = bits.enumerate().filter(|(_, b)| *b).map(|(i, _)| i as u32).collect();
                Every::Month(days)
            },
            Ok("year") => {
                let arr: Vec<i32> = match d.remove("deps") {
                    Some(Bson::Array(a)) => a.into_iter().map(|d| d.as_i32().unwrap()).collect(),
                    _ => panic!("expected array for 'data' with type 'year'"),
                };
                let mut arr = arr.into_iter();
                let mut res = Vec::new();
                while let (Some(month), Some(days)) = (arr.next(), arr.next()) {
                    let m = Month::from_num(month as usize);
                    let bits = bits(days as u32, 31);
                    let days = bits.enumerate().filter(|(_, b)| *b).map(|(i, _)| i as u32).collect();
                    res.push((m, days));
                }
                Every::Year(res)
            },
            _ => panic!("Unknown 'head' type when deserializing `Every`: {}", d),
        }
    }
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum Stop {
    None,
    Count(i32),
    After(DateTime),
}

impl Stop {
    pub fn to_doc(self) -> Document {
        match self {
            Stop::None => doc! { "head": "none" },
            Stop::Count(i) => doc! { "head": "count", "data": i },
            Stop::After(t) => doc! { "head": "after", "data": t.to_utc() },
        }
    }

    pub fn from_doc(d: Document) -> Stop {
        match d.get_str("head") {
            Ok("none") => Stop::None,
            Ok("count") => Stop::Count(d.get_i32("data").expect("expected `i32` for 'data'")),
            Ok("after") => Stop::After(d.get_datetime("data").expect("expected `DateTime` for 'data'").clone().into()),
            _ => panic!("Unknown 'head' type when deserializing `Stop`: {}", d),
        }
    }
}
