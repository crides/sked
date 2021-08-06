use clap::{App, AppSettings::*};
use rustyline::{error::ReadlineError, Editor};

pub fn cmd_repl() {
    let mut editor = Editor::<()>::new();
    let mut cmds = App::new("cmd").settings(&[NoBinaryName]);
    loop {
        match editor.readline(">=> ") {
            Ok(line) => {
                let line = line.trim();
                if !line.is_empty() {
                    editor.add_history_entry(line);
                }
                let args = line.split_ascii_whitespace();
                match cmds.get_matches_from_safe_borrow(args) {
                    Ok(matches) => match matches.subcommand() {
                        (_, _) => panic!(),
                    },
                    Err(e) => {
                        eprintln!("{}", e.message);
                    }
                }
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("{:?}", e);
                break;
            }
        }
    }
}
