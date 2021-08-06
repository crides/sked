#[cfg(features = "scripting")]
use gluon::vm::api::{OwnedFunction, IO};
use regex::Regex;

use crate::storage::api::ScriptLog;
use crate::storage::{Error, Result};
#[cfg(features = "scripting")]
use crate::util::print_gluon_err;

#[cfg(features = "scripting")]
pub type GluonHandler = OwnedFunction<fn(ScriptLog) -> IO<()>>;
pub type NativeHandler = &'static (dyn Send + Sync + Fn(ScriptLog) -> Result<()>);

pub enum LogHandler {
    #[cfg(features = "scripting")]
    Gluon(GluonHandler),
    Native(NativeHandler),
}

pub struct LogHandlerEntry {
    pat: Regex,
    func: LogHandler,
}

pub struct LogHandlers(Vec<LogHandlerEntry>);

impl LogHandlers {
    pub fn new() -> LogHandlers {
        LogHandlers(Vec::new())
    }

    #[cfg(features = "scripting")]
    pub fn add_gluon(&mut self, pat: &str, func: GluonHandler) -> Result<()> {
        self.0.push(LogHandlerEntry {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func: LogHandler::Gluon(func),
        });
        Ok(())
    }

    pub fn add_native(&mut self, pat: &str, func: NativeHandler) -> Result<()> {
        self.0.push(LogHandlerEntry {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func: LogHandler::Native(func),
        });
        Ok(())
    }

    pub fn handle(&mut self, l: &ScriptLog) {
        for handler in &mut self.0 {
            if handler.pat.is_match(&l.typ) {
                match handler.func {
                    #[cfg(features = "scripting")]
                    LogHandler::Script(f) => {
                        if let Err(e) = handler.func.call(l.clone()) {
                            eprintln!("Error running log handler:");
                            print_gluon_err(e);
                        }
                    }
                    LogHandler::Native(f) => {
                        if let Err(e) = f(l.clone()) {
                            eprintln!("Error running log handler: {}", e);
                        }
                    }
                }
            }
        }
    }
}
