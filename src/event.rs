use regex::Regex;
use rlua::prelude::*;

use crate::storage::{Error, Log, Result};

pub struct EventHandler<'lua> {
    pat: Regex,
    func: LuaFunction<'lua>,
}

pub struct EventHandlers<'lua>(Vec<EventHandler<'lua>>);

impl<'lua> EventHandlers<'lua> {
    pub fn new() -> EventHandlers<'lua> {
        EventHandlers(Vec::new())
    }

    pub fn add_lua(&mut self, pat: &str, f: LuaFunction<'lua>) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat).map_err(|_| Error::Regex(pat.to_string()))?,
            func: f,
        });
        Ok(())
    }

    pub fn handle(&self, l: &Log) {
        for handler in &self.0 {
            if handler.pat.is_match(&l.typ) {
                let _ = handler.func.call::<_, ()>((l.clone(),));
            }
        }
    }
}
