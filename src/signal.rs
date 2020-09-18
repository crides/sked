use gluon::vm::api::{OwnedFunction, IO};
use regex::Regex;

use crate::script::sched::Log;
use crate::storage::{Error, Result};
use crate::util::print_gluon_err;

pub type SignalHandler = OwnedFunction<fn(Log) -> IO<()>>;

pub struct SignalHandlerEntry {
    pat: Regex,
    func: SignalHandler,
}

pub struct SignalHandlers(Vec<SignalHandlerEntry>);

impl SignalHandlers {
    pub fn new() -> SignalHandlers {
        SignalHandlers(Vec::new())
    }

    pub fn add_gluon(&mut self, pat: &str, func: SignalHandler) -> Result<()> {
        self.0.push(SignalHandlerEntry {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func,
        });
        Ok(())
    }

    pub fn handle(&mut self, l: &Log) {
        for handler in &mut self.0 {
            if handler.pat.is_match(&l.typ) {
                if let Err(e) = handler.func.call(l.clone()) {
                    eprintln!("Error running signal handler:");
                    print_gluon_err(e.into());
                }
            }
        }
    }
}
