#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate thiserror;

mod script;
mod storage;

use std::fs;

use dirs::config_dir;

use script::run_script;

fn main() {
    let config_dir = config_dir().unwrap().join("sched");
    if !config_dir.is_dir() {
        fs::create_dir_all(&config_dir).unwrap();
    }
    run_script(&config_dir).unwrap();
}
