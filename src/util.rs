#[cfg(features = "scripting")]
use codespan_reporting::term::termcolor::{ColorChoice::Always, StandardStream};

#[cfg(features = "scripting")]
pub fn print_gluon_err(e: gluon::Error) {
    e.emit(&mut StandardStream::stderr(Always)).unwrap();
}
