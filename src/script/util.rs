//! All the Lua helper functions/APIs that doesn't have anything to do with the core Logging and
//! Event APIs.

use gluon::{Thread, ThreadExt};
use rustyline::{Config, Editor};

pub fn repl(thread: &Thread) {
    let mut editor = Editor::<()>::with_config(Config::builder().tab_stop(4).build());
    loop {
        let mut prompt = "> ";
        let mut line = String::new();
        loop {
            match editor.readline(prompt) {
                Ok(input) => line.push_str(&input),
                Err(_) => return,
            }

            match thread.run_expr::<()>("repl", &line) {
                Ok((val, _)) => {
                    editor.add_history_entry(line);
                    // println!("{}", val);
                    break;
                }
                // Err(LuaError::SyntaxError {
                //     incomplete_input: true,
                //     ..
                // }) => {
                //     // continue reading input and append it to `line`
                //     line.push_str("\n"); // separate input lines
                //     prompt = ">> ";
                // }
                Err(e) => {
                    eprintln!("error: {}", e);
                    break;
                }
            }
        }
    }
}
