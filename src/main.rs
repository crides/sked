#[macro_use]
extern crate gluon_codegen;
#[macro_use]
extern crate gluon;
#[macro_use]
extern crate serde_derive;

mod repl;
mod script;
mod signal;
mod storage;
mod task;
mod util;

use std::fs;
use std::time::Duration;

use dirs::config_dir;
use tokio::{select, time::delay_for};

use script::job;
use util::print_gluon_err;

#[tokio::main]
async fn main() {
    let config_dir = config_dir().unwrap().join("sched");
    if !config_dir.is_dir() {
        fs::create_dir_all(&config_dir).unwrap();
    }
    let init_file = config_dir.join("init.glu");
    let vm = script::get_vm(config_dir);
    if let Err(e) = script::run_user(&vm, &init_file) {
        print_gluon_err(e);
    }
    let job_task = tokio::task::spawn(async {
        loop {
            job::run();
            delay_for(Duration::from_millis(200)).await;
        }
    });
    let cmd_repl = async { script::cmd::cmd_repl() };

    select! {
        _ = job_task => (),
        r = cmd_repl => {
            if r {
                let res = repl::run(&vm, "> ").await;
                if let Err(e) = res {
                    print_gluon_err(e);
                }
            }
        },
    }
}
