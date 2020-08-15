use anyhow::Result;
use either::Either;
use regex::Regex;
use rlua::prelude::*;

use crate::storage::Log;

pub struct EventHandler<'lua> {
    pat: Regex,
    func: Either<Box<dyn Fn(&Log) + Send>, LuaFunction<'lua>>,
}

pub struct EventHandlers<'lua>(Vec<EventHandler<'lua>>);

impl<'lua> EventHandlers<'lua> {
    pub fn new() -> EventHandlers<'lua> {
        EventHandlers(Vec::new())
    }

    pub fn add(&mut self, pat: &str, f: Box<dyn Fn(&Log) + Send>) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat)?,
            func: Either::Left(f),
        });
        Ok(())
    }

    pub fn add_lua(&mut self, pat: &str, f: LuaFunction<'lua>) -> Result<()> {
        self.0.push(EventHandler {
            pat: Regex::new(pat)?,
            func: Either::Right(f),
        });
        Ok(())
    }

    pub fn handle(&self, l: &Log) {
        for handler in &self.0 {
            if handler.pat.is_match(&l.typ) {
                match &handler.func {
                    Either::Left(ref f) => f(l),
                    Either::Right(ref f) => {
                        let _ = f.call::<_, ()>((l.clone(),));
                    }
                }
            }
        }
    }
}
