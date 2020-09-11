#[macro_use]
extern crate gluon_codegen;
#[macro_use]
extern crate gluon;
#[macro_use]
extern crate serde_derive;

mod signal;
mod repl;
mod script;
mod storage;

use std::fs;

use dirs::config_dir;

use script::ScriptContext;

#[tokio::main(basic_scheduler)]
async fn main() {
    let config_dir = config_dir().unwrap().join("sched");
    if !config_dir.is_dir() {
        fs::create_dir_all(&config_dir).unwrap();
    }
    let script_ctx = ScriptContext::new();
    if let Err(e) = script_ctx.init_user(config_dir) {
        println!("{}", e);
    }
    if let Err(e) = repl::run(&script_ctx.vm, "> ").await {
        println!("{}", e);
    }
}
