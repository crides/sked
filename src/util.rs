use codespan_reporting::term::termcolor::{ColorChoice::Always, StandardStream};

pub struct Bits {
    data: u32,
    rem: u8,
}

impl Iterator for Bits {
    type Item = bool;
    fn next(&mut self) -> Option<bool> {
        if self.rem > 0 {
            let b = (self.data & 1) != 0;
            self.rem -= 1;
            self.data >>= 1;
            Some(b)
        } else {
            None
        }
    }
}

pub fn bits(data: u32, len: u8) -> Bits {
    Bits { data, rem: len }
}

pub fn print_gluon_err(e: gluon::Error) {
    e.emit(&mut StandardStream::stderr(Always)).unwrap();
}
