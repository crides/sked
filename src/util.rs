use codespan_reporting::term::termcolor::{ColorChoice::Always, StandardStream};

pub fn print_gluon_err(e: gluon::Error) {
    e.emit(&mut StandardStream::stderr(Always)).unwrap();
}
