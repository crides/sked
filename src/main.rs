#[cfg(features = "scripting")]
#[macro_use]
extern crate gluon_codegen;
#[cfg(features = "scripting")]
#[macro_use]
extern crate gluon;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derive_new;

mod handler;
mod notify;
#[cfg(features = "repl")]
mod repl;
#[cfg(features = "scripting")]
mod script;
mod storage;
mod util;

#[cfg(not(features = "scripting"))]
#[path = "native_repl.rs"]
mod repl;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{App, Arg};
use dirs::config_dir;
use tokio::sync::Notify;

use storage::STORE;

#[tokio::main(threaded_scheduler)]
async fn main() {
    let config_dir = config_dir().unwrap().join("sched");
    if !config_dir.is_dir() {
        fs::create_dir_all(&config_dir).unwrap();
    }
    let matches = App::new("sched")
        .arg(Arg::with_name("init-file").required(false))
        .subcommand(App::new("export").arg(Arg::with_name("file").required(true)))
        .subcommand(App::new("import").arg(Arg::with_name("file").required(true)))
        .get_matches();
    // FIXME handle IO errors
    match matches.subcommand() {
        ("export", Some(m)) => {
            let file = m.value_of("file").unwrap();
            let data = STORE.export();
            let mut file = File::create(file).unwrap();
            serde_json::to_writer_pretty(&mut file, &data).unwrap();
        }
        ("import", Some(m)) => {
            let file = m.value_of("file").unwrap();
            let content = fs::read_to_string(file).unwrap();
            STORE.import(&content);
        }
        ("", _) => {
            #[cfg(features = "scripting")]
            let init_file: PathBuf = matches
                .value_of("init-file")
                .map_or_else(|| config_dir.join("init.glu"), |s| s.into());
            let quit_sig = Arc::new(Notify::new());
            let notify_task = tokio::spawn(notify_loop(quit_sig.clone()));
            tokio::task::block_in_place(move || {
                repl_loop(
                    #[cfg(features = "scripting")]
                    &init_file,
                );
                quit_sig.notify();
            });
            notify_task.await.unwrap();
        }
        _ => unreachable!(),
    }
}

async fn notify_loop(quit_sig: Arc<Notify>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(3000));
    loop {
        notify::notify();
        tokio::select! {
            _ = interval.tick() => (),
            _ = quit_sig.notified() => {
                break;
            },
        }
    }
}

#[cfg(not(features = "scripting"))]
fn repl_loop() {}

#[cfg(features = "scripting")]
fn repl_loop(init_file: &Path) {
    use util::print_gluon_err;
    let init_dir = init_file.parent().unwrap();
    let vm = script::get_vm(init_dir.to_owned());
    if let Err(e) = script::run_user(&vm, &init_file) {
        print_gluon_err(e);
        return;
    }
    if script::cmd::cmd_repl() {
        let res = repl::run(&vm, "> ");
        if let Err(e) = res {
            print_gluon_err(e);
        }
    }
}
