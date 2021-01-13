use gluon::{vm::ExternModule, Thread};

fn split<'a>(s: &'a str, sep: &'a str) -> Vec<&'a str> {
    s.split(sep).collect()
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            split => primitive!(2, split),
        },
    )
}
