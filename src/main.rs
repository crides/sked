#[macro_use]
extern crate serde_derive;

mod event;
mod script;
mod storage;

use std::fs;

use dirs::config_dir;

use script::ScriptContext;

fn main() {
    let config_dir = config_dir().unwrap().join("sched");
    if !config_dir.is_dir() {
        fs::create_dir_all(&config_dir).unwrap();
    }
    let mut script_ctx = ScriptContext::new().unwrap();
    script_ctx.init_lib().unwrap();
    script_ctx.init_user(config_dir).unwrap();
}
