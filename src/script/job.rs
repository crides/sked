use std::sync::Mutex;

use chrono::{DateTime, Duration, NaiveDateTime, Offset, Utc};
use gluon::{
    vm::{
        api::{OwnedFunction, IO},
        ExternModule,
    },
    Thread,
};
use lazy_static::lazy_static;

use crate::script::time::{DateTime as GluonDateTime, Duration as GluonDuration};
use crate::util::print_gluon_err;

type TimedFunc = OwnedFunction<fn(GluonDateTime) -> IO<()>>;
type CountedFunc = OwnedFunction<fn(u32) -> IO<()>>;
type NextTimeFunc = OwnedFunction<fn(GluonDateTime) -> Option<GluonDateTime>>;

// `start` and `stop` are always UTC time
enum Job {
    Until {
        start: NaiveDateTime,
        interval: Duration,
        stop: NaiveDateTime,
        job: TimedFunc,
    },
    Counted {
        start: NaiveDateTime,
        interval: Duration,
        count: u32,
        job: CountedFunc,
    },
    Custom {
        start: NaiveDateTime,
        next: NextTimeFunc,
        job: TimedFunc,
    },
}

lazy_static! {
    static ref JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());
}

pub fn run() {
    let mut jobs = JOBS.lock().unwrap();
    *jobs = jobs
        .drain(..)
        .filter_map(|j| {
            let now = Utc::now().naive_utc();
            let next = match j {
                Job::Counted { start, .. } | Job::Until { start, .. } | Job::Custom { start, .. } => start,
            };
            if next > now {
                return Some(j);
            }
            match j {
                Job::Counted {
                    interval,
                    count,
                    mut job,
                    ..
                } => {
                    let count = count - 1;
                    if let Err(e) = job.call(count) {
                        eprintln!("Error running job handler:");
                        print_gluon_err(e.into());
                    }
                    if count > 0 {
                        Some(Job::Counted {
                            start: next + interval,
                            interval,
                            count,
                            job,
                        })
                    } else {
                        None
                    }
                }
                Job::Until {
                    start,
                    interval,
                    stop,
                    mut job,
                } => {
                    let next = start + interval;
                    if let Err(e) = job.call(GluonDateTime(DateTime::from_utc(now, Utc.fix()))) {
                        eprintln!("Error running job handler:");
                        print_gluon_err(e.into());
                    }
                    if next < stop {
                        Some(Job::Until {
                            start: next,
                            interval,
                            stop,
                            job,
                        })
                    } else {
                        None
                    }
                }
                Job::Custom { mut next, mut job, .. } => {
                    let now = GluonDateTime(DateTime::from_utc(now, Utc.fix()));
                    if let Err(e) = job.call(now) {
                        eprintln!("Error running job handler:");
                        print_gluon_err(e.into());
                    }
                    if let Some(time) = next.call(now).unwrap() {
                        Some(Job::Custom {
                            start: time.0.naive_utc(),
                            next,
                            job,
                        })
                    } else {
                        None
                    }
                }
            }
        })
        .collect();
}

fn counted_at(time: GluonDateTime, interval: GluonDuration, count: u32, job: CountedFunc) {
    if count == 0 {
        return;
    }
    JOBS.lock().unwrap().push(Job::Counted {
        start: time.0.naive_utc(),
        interval: interval.0,
        count,
        job,
    });
}

fn until_at(time: GluonDateTime, interval: GluonDuration, stop: GluonDateTime, job: TimedFunc) {
    if stop.0 < time.0 {
        return;
    }
    JOBS.lock().unwrap().push(Job::Until {
        start: time.0.naive_utc(),
        interval: interval.0,
        stop: stop.0.naive_utc(),
        job,
    });
}

fn custom_at(time: GluonDateTime, next: NextTimeFunc, job: TimedFunc) {
    JOBS.lock().unwrap().push(Job::Custom {
        start: time.0.naive_utc(),
        next,
        job,
    });
}

fn counted_now(count: u32, interval: GluonDuration, job: CountedFunc) {
    if count == 0 {
        return;
    }
    JOBS.lock().unwrap().push(Job::Counted {
        start: Utc::now().naive_utc(),
        interval: interval.0,
        count,
        job,
    });
}

fn until_now(stop: GluonDateTime, interval: GluonDuration, job: TimedFunc) {
    if stop.0 < Utc::now() {
        return;
    }
    JOBS.lock().unwrap().push(Job::Until {
        start: Utc::now().naive_utc(),
        interval: interval.0,
        stop: stop.0.naive_utc(),
        job,
    });
}

fn custom_now(next: NextTimeFunc, job: TimedFunc) {
    JOBS.lock().unwrap().push(Job::Custom {
        start: Utc::now().naive_utc(),
        next,
        job,
    });
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            counted_at => primitive!(4, counted_at),
            until_at => primitive!(4, until_at),
            custom_at => primitive!(3, custom_at),
            counted_now => primitive!(3, counted_now),
            until_now => primitive!(3, until_now),
            custom_now => primitive!(2, custom_now),
        },
    )
}
