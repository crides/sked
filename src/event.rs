use either::Either;
use gluon::vm::api::{OwnedFunction, IO};
use regex::Regex;

use crate::storage::{Error, Log, Result};

pub type Handler = OwnedFunction<fn(Log) -> IO<()>>;

pub struct EventHandler {
    pat: Regex,
    func: Either<Box<dyn Fn(&Log) + Send>, Handler>,
}

pub struct EventHandlers(Vec<EventHandler>);

impl EventHandlers {
    pub fn new() -> EventHandlers {
        EventHandlers(Vec::new())
    }

    pub fn add(&mut self, pat: &str, f: Box<dyn Fn(&Log) + Send>) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func: Either::Left(f),
        });
        Ok(())
    }

    pub fn add_gluon(&mut self, pat: &str, f: Handler) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func: Either::Right(f),
        });
        Ok(())
    }

    pub fn handle(&mut self, l: &Log) {
        for handler in &mut self.0 {
            if handler.pat.is_match(&l.typ) {
                match &mut handler.func {
                    Either::Left(ref f) => f(l),
                    Either::Right(ref mut f) => {
                        f.call(l.clone()).unwrap();
                    }
                }
            }
        }
    }
}
