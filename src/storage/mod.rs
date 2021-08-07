pub mod api;
mod kv;
mod macros;
pub mod time;

pub use kv::*;
pub use macros::*;

use chrono::Datelike;
use lazy_static::lazy_static;
use thiserror::Error;

use crate::storage::{
    api::{LogId, ObjId},
    time::{DateTime, Duration},
};

lazy_static! {
    pub static ref STORE: Storage = Storage::new();
}

// FIXME define types (newtype?) for log and object IDs
#[derive(Debug, Error)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Pushable, Getable), gluon_trace(skip))]
pub enum Error {
    #[error("Deadlock")]
    Deadlock,
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
    #[error("Can't compile regex '{0}'")]
    Regex(String),
    #[error("Invalid Log ID {0}")]
    InvalidLogID(LogId),
    #[error("Invalid Object ID {0}")]
    InvalidObjID(ObjId),
    #[error("Type mismatch with stored, expected: '{expected}', actual: '{actual}'")]
    TypeMismatch { expected: String, actual: String },
    #[error("Cannot delete an non-existent attribute '{1}' at object '{0}'")]
    DelNonExistent(ObjId, String),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// FIXME manually implement `Pushable` and `Getable`, so that internal state is not passed to Gluon, and that
// they are set to reset state when passed from Gluon
// FIXME use other internal states to record when to stop i.e. can't change the stop properties for public
// interface
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
pub struct Repeated {
    /// A set of actual start times
    start: Vec<DateTime>,
    every: Every,
    stop: Stop,

    last: Option<DateTime>,
    /// Index into `start`, for from which start time the current time is derived from
    index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[serde(rename_all = "lowercase")]
pub enum OptRepeated {
    Single(DateTime),
    Repeat(Repeated),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[serde(rename_all = "lowercase")]
pub enum Every {
    Time(Duration),
    Month(u32),
}

impl Every {
    fn advance(&self, time: DateTime) -> DateTime {
        let time = match self {
            Every::Time(dur) => time.0 + dur.0,
            Every::Month(c) => {
                let month = time.0.month0() + c;
                let (year_diff, month) = (month / 12, month % 12);
                time.0
                    .with_year(time.0.year() + year_diff as i32)
                    .unwrap()
                    .with_month0(month)
                    .expect(&format!("invalid month {}", month))
            }
        };
        DateTime(time)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[serde(rename_all = "lowercase")]
pub enum Stop {
    Nonstop,
    Stopped,
    Count(i32),
    After(DateTime),
}

// TODO `start[0]` + `every` < `start[n]`?
impl Repeated {
    pub fn new(mut start: Vec<DateTime>, every: Every, stop: Stop) -> Repeated {
        start.sort();
        Repeated {
            start,
            every,
            stop,
            last: None,
            index: 0,
        }
    }
}

impl Iterator for Repeated {
    type Item = DateTime;
    fn next(&mut self) -> Option<DateTime> {
        match self.stop {
            Stop::Count(count) => {
                if count == 0 {
                    self.stop = Stop::Stopped;
                } else {
                    self.stop = Stop::Count(count - 1);
                }
            }
            _ => (),
        }
        if matches!(self.stop, Stop::Stopped) {
            return None;
        }
        // Assuming `start` is sorted
        if let Some(DateTime(mut last)) = self.last {
            if self.index == self.start.len() - 1 {
                last = last - (self.start[self.start.len() - 1].0 - self.start[0].0);
                last = self.every.advance(DateTime(last)).0;
                self.index = 0;
            } else {
                self.index += 1;
                last = last + (self.start[self.index].0 - self.start[self.index - 1].0);
            }
            self.last = Some(DateTime(last));
        } else {
            self.last = Some(self.start[0]);
        }
        if let Stop::After(time) = self.stop {
            if time.0 < self.last.unwrap().0 {
                self.stop = Stop::Stopped;
                return None;
            }
        }
        self.last
    }
}

#[cfg(test)]
mod test {
    use chrono::prelude::*;
    use chrono::Duration;

    fn datetime(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> super::DateTime {
        super::DateTime(Utc.ymd(y, m, d).and_hms(h, mi, s).into())
    }

    #[test]
    fn test_every_advance() {
        use super::Every;
        let now = datetime(2020, 12, 25, 12, 13, 14);
        assert_eq!(
            Every::Time(Duration::days(3).into()).advance(now),
            datetime(2020, 12, 28, 12, 13, 14)
        );
        assert_eq!(
            Every::Time(Duration::days(7).into()).advance(now),
            datetime(2021, 1, 1, 12, 13, 14)
        );
        assert_eq!(
            Every::Time(Duration::weeks(1).into()).advance(now),
            datetime(2021, 1, 1, 12, 13, 14)
        );
        assert_eq!(Every::Month(1).advance(now), datetime(2021, 1, 25, 12, 13, 14));
        assert_eq!(Every::Month(12).advance(now), datetime(2021, 12, 25, 12, 13, 14));
        assert_eq!(Every::Month(18).advance(now), datetime(2022, 6, 25, 12, 13, 14));
    }

    #[test]
    fn test_repeat() {
        use super::{Every, Repeated, Stop};
        let repeat = Repeated::new(
            vec![
                datetime(2020, 12, 21, 10, 0, 0),
                datetime(2020, 12, 23, 11, 0, 0),
                datetime(2020, 12, 25, 12, 0, 0),
            ],
            Every::Time(Duration::weeks(1).into()),
            Stop::Count(7),
        );
        assert_eq!(
            repeat.collect::<Vec<_>>(),
            vec![
                datetime(2020, 12, 21, 10, 0, 0),
                datetime(2020, 12, 23, 11, 0, 0),
                datetime(2020, 12, 25, 12, 0, 0),
                datetime(2020, 12, 28, 10, 0, 0),
                datetime(2020, 12, 30, 11, 0, 0),
                datetime(2021, 1, 1, 12, 0, 0),
                datetime(2021, 1, 4, 10, 0, 0),
            ]
        );
        let repeat = Repeated::new(
            vec![
                datetime(2020, 12, 21, 10, 0, 0),
                datetime(2020, 12, 23, 11, 0, 0),
                datetime(2020, 12, 25, 12, 0, 0),
            ],
            Every::Time(Duration::weeks(1).into()),
            Stop::After(datetime(2021, 1, 4, 11, 0, 0)),
        );
        assert_eq!(
            repeat.collect::<Vec<_>>(),
            vec![
                datetime(2020, 12, 21, 10, 0, 0),
                datetime(2020, 12, 23, 11, 0, 0),
                datetime(2020, 12, 25, 12, 0, 0),
                datetime(2020, 12, 28, 10, 0, 0),
                datetime(2020, 12, 30, 11, 0, 0),
                datetime(2021, 1, 1, 12, 0, 0),
                datetime(2021, 1, 4, 10, 0, 0),
            ]
        );
    }
}
