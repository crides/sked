use gluon::vm::api::{OwnedFunction, IO};
use regex::Regex;

use crate::storage::{Error, Log, Result};

pub type Handler = OwnedFunction<fn(Log) -> IO<()>>;

pub struct EventHandler {
    pat: Regex,
    func: Handler,
}

pub struct EventHandlers(Vec<EventHandler>);

impl EventHandlers {
    pub fn new() -> EventHandlers {
        EventHandlers(Vec::new())
    }

    pub fn add_gluon(&mut self, pat: &str, func: Handler) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func,
        });
        Ok(())
    }

    pub fn handle(&mut self, l: &Log) {
        for handler in &mut self.0 {
            if handler.pat.is_match(&l.typ) {
                handler.func.call(l.clone()).unwrap();
            }
        }
    }
}
