use std::collections::BTreeMap;

use gluon::{
    vm::{
        api::{FunctionRef, IO},
        ExternModule,
    },
    Thread,
};

use crate::{
    script::{
        task::{Event, Task},
        time::DateTime,
    },
    storage::{
        api::{AttrValue, Attrs, ProtoLog},
        Error, Every, OptRepeated, Repeated, Result as StorageResult, Stop, Storage,
    },
};

fn lalign(s: &str, n: usize) -> String {
    format!("{}{}", s, " ".repeat(n - s.len()))
}

fn ralign(s: &str, n: usize) -> String {
    format!("{}{}", " ".repeat(n - s.len()), s)
}

mod log {
    use super::{lalign, ralign};
    use crate::storage::{
        api::{AttrValue, Attrs, ProtoLog},
        Result as StorageResult, STORE,
    };
    use gluon::vm::api::{FunctionRef, IO};
    fn new(typ: String, attrs: Attrs) -> StorageResult<u32> {
        STORE.create_log(typ, attrs)
    }

    fn get(id: u32) -> StorageResult<ProtoLog> {
        STORE.get_log(id)
    }

    fn find(filter: FunctionRef<fn(ProtoLog) -> bool>, limit: Option<usize>) -> StorageResult<Vec<ProtoLog>> {
        Ok(STORE.find_log(|l| filter.clone().call(l.clone()).unwrap(), limit))
    }

    fn find_from(
        id: u32,
        filter: FunctionRef<fn(ProtoLog) -> bool>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<ProtoLog>> {
        Ok(STORE.find_log_from(id, |l| filter.clone().call(l.clone()).unwrap(), limit))
    }

    fn find_old(filter: FunctionRef<fn(ProtoLog) -> bool>, limit: Option<usize>) -> StorageResult<Vec<ProtoLog>> {
        Ok(STORE.find_log_old(|l| filter.clone().call(l.clone()).unwrap(), limit))
    }

    fn find_old_from(
        id: u32,
        filter: FunctionRef<fn(ProtoLog) -> bool>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<ProtoLog>> {
        Ok(STORE.find_log_old_from(id, |l| filter.clone().call(l.clone()).unwrap(), limit))
    }

    fn list(num: usize) -> IO<()> {
        // TODO fix this table rendering
        let logs = STORE.find_log(|_l| true, Some(num));
        let header = (
            "id".to_string(),
            "typ".to_string(),
            "time".to_string(),
            "attrs".to_string(),
        );
        let rows = logs
            .iter()
            .map(|l| {
                (
                    l.id.to_string(),
                    l.typ.clone(),
                    l.time.format("%Y.%m.%d..%H.%M.%S"),
                    l.attrs.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let longest = std::iter::once((header.0.len(), header.1.len(), header.2.len(), header.3.len()))
            .chain(rows.iter().map(|r| {
                (
                    r.0.len(),
                    r.1.len(),
                    r.2.len(),
                    r.3.iter().map(|line| line.len()).max().unwrap_or(0),
                )
            }))
            .fold((0, 0, 0, 0), |(a1, b1, c1, d1), (a2, b2, c2, d2)| {
                use std::cmp::max;
                (max(a1, a2), max(b1, b2), max(c1, c2), max(d1, d2))
            });
        let column_spacing = 2;
        use termion::{
            color::{self, *},
            style::{self, *},
        };
        println!(
            "{}{}{}{space}{}{space}{}{space}{}{}{}",
            Fg(White),
            Bold,
            lalign(&header.0, longest.0),
            lalign(&header.1, longest.1),
            lalign(&header.2, longest.2),
            lalign(&header.3, longest.3),
            Fg(color::Reset),
            style::Reset,
            space = " ".repeat(column_spacing)
        );
        for r in rows {
            let first_attr = r.3.first().map(|s| s.as_str()).unwrap_or("");
            println!(
                "{}{}{}{space}{}{space}{}{space}{}{}{}",
                Fg(Green),
                ralign(&r.0, longest.0),
                Fg(color::Reset),
                lalign(&r.1, longest.1),
                lalign(&r.2, longest.2),
                Fg(Blue),
                lalign(first_attr, longest.3),
                Fg(color::Reset),
                space = " ".repeat(column_spacing)
            );
            if r.3.len() > 1 {
                let pre_space = " ".repeat(longest.0 + longest.1 + longest.2 + 3 * column_spacing);
                for attr in r.3.iter().skip(1) {
                    println!("{}{}{}{}", pre_space, Fg(Blue), attr, Fg(color::Reset));
                }
                println!("");
            }
        }
        IO::Value(())
    }
}

mod obj {
    use super::{lalign, ralign};
    use crate::{
        script::sched::STORE,
        storage::{
            api::{AttrValue, Attrs, ProtoObj},
            Result as StorageResult,
        },
    };
    use gluon::vm::api::{FunctionRef, IO};
    fn new(name: String, typ: String) -> StorageResult<ProtoObj> {
        let id = STORE.create_obj(&name, &typ)?;
        Ok(Object {
            id,
            name,
            typ,
            desc: None,
            attrs: None,
        })
    }

    fn get(id: u32) -> StorageResult<Object> {
        STORE.get_obj(id)
    }

    fn set_desc(obj: Object, desc: &str) -> StorageResult<()> {
        STORE.obj_set_desc(obj.id, desc.to_string())
    }

    fn set_attr(obj: Object, key: &str, val: AttrValue) -> StorageResult<()> {
        STORE.obj_set_attr(obj.id, key.to_string(), val)
    }

    fn del_attr(obj: Object, attr: &str) -> StorageResult<()> {
        STORE.obj_del_attr(obj.id, attr)
    }

    fn find(filter: FunctionRef<fn(Object) -> bool>, limit: Option<usize>) -> StorageResult<Vec<Object>> {
        Ok(STORE.find_obj(|o| filter.clone().call(o.clone()).unwrap(), limit))
    }

    fn find_from(id: u32, filter: FunctionRef<fn(Object) -> bool>, limit: Option<usize>) -> StorageResult<Vec<Object>> {
        Ok(STORE.find_obj_from(id, |o| filter.clone().call(o.clone()).unwrap(), limit))
    }

    fn find_old(filter: FunctionRef<fn(Object) -> bool>, limit: Option<usize>) -> StorageResult<Vec<Object>> {
        Ok(STORE.find_obj_old(|o| filter.clone().call(o.clone()).unwrap(), limit))
    }

    fn find_old_from(
        id: u32,
        filter: FunctionRef<fn(Object) -> bool>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Object>> {
        Ok(STORE.find_obj_old_from(id, |o| filter.clone().call(o.clone()).unwrap(), limit))
    }
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            type Every => Every,
            type Stop => Stop,
            type Repeated => Repeated,
            type OptRepeated => OptRepeated,
            type Error => Error,
            log => record! {
                type ProtoLog => ProtoLog,
                new => primitive!(2, Log::new),
                get => primitive!(1, Log::get),
                set_attr => primitive!(3, Log::set_attr),
                find => primitive!(2, Log::find),
                find_from => primitive!(3, Log::find_from),
                find_old => primitive!(2, Log::find_old),
                find_old_from => primitive!(3, Log::find_old_from),
                list => primitive!(1, Log::list),
            },

            obj => record! {
                type ProtoObj => ProtoObj,
                new => primitive!(2, Object::new),
                get => primitive!(1, Object::get),
                set_desc => primitive!(2, Object::set_desc),
                set_attr => primitive!(3, Object::set_attr),
                del_attr => primitive!(2, Object::del_attr),
                find => primitive!(2, Object::find),
                find_from => primitive!(3, Object::find_from),
                find_old => primitive!(2, Object::find_old),
                find_old_from => primitive!(3, Object::find_old_from),
            },

            task => record! {
                type Task => Task,
                new => primitive!(4, Task::new),
                get => primitive!(1, Task::get),
                finish => primitive!(1, Task::finish),
                find_current => primitive!(1, Task::find_current),
            },

            event => record! {
                type Event => Event,
                new => primitive!(4, Event::new),
                get => primitive!(1, Event::get),
            },

            handle => primitive!(2, |pat, func| {
                STORE.add_gluon(pat, func)
            }),
            repeat => primitive!(3, |start, every, stop| {
                Repeated::new(start, every, stop)
            }),
        },
    )
}
