use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, TryLockError};

use gluon::{
    vm::{
        api::{FunctionRef, IO},
        ExternModule,
    },
    Thread,
};
use lazy_static::lazy_static;
pub use serde_json::Value as AttrValue;

use crate::{
    script::{
        task::{Event, Task},
        time::DateTime,
    },
    storage::{Error, Every, OptRepeated, Repeated, Result as StorageResult, Stop, Storage},
};

lazy_static! {
    pub static ref STORE: Mutex<Storage> = Mutex::new(Storage::new());
}

pub type Attrs = BTreeMap<String, AttrValue>;

#[derive(Clone, Debug, VmType, Pushable, Getable)]
#[cfg_attr(feature = "mongo", derive(Deserialize))]
pub struct Log {
    #[cfg_attr(feature = "mongo", serde(rename(deserialize = "_id")))]
    pub id: u32,
    pub typ: String,
    pub time: DateTime,
    #[cfg_attr(feature = "mongo", serde(default))]
    pub attrs: Attrs,
}

pub fn lock_store() -> StorageResult<MutexGuard<'static, Storage>> {
    // FIXME Restrict locks to only log handlers; `Tree`s are protected by inner locks
    // Also maybe record call stack of handlers for avoiding recursive bugs?
    match STORE.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(Error::Deadlock),
        Err(TryLockError::Poisoned(_)) => panic!("STORE lock poisoned"),
    }
}

fn lalign(s: &str, n: usize) -> String {
    format!("{}{}", s, " ".repeat(n - s.len()))
}

fn ralign(s: &str, n: usize) -> String {
    format!("{}{}", " ".repeat(n - s.len()), s)
}

macro_rules! try_io {
    ( $e:expr ) => {
        match $e {
            Ok(ok) => ok,
            Err(err) => return ::gluon::vm::api::IO::Exception(err.to_string()),
        }
    };
}

impl Log {
    fn new(typ: String, attrs: Attrs) -> StorageResult<u32> {
        lock_store()?.create_log(typ, attrs)
    }

    fn get(id: u32) -> StorageResult<Log> {
        lock_store()?.get_log(id)
    }

    fn set_attr(self, key: String, val: AttrValue) -> StorageResult<()> {
        lock_store()?.log_add_attr(self.id, key, val)
    }

    fn find(filter: FunctionRef<fn(Log) -> bool>, limit: Option<usize>) -> StorageResult<Vec<Log>> {
        Ok(lock_store()?.find_log(|l| filter.clone().call(l.clone()).unwrap(), limit))
    }

    fn list(num: usize) -> IO<()> {
        let logs = try_io!(lock_store()).find_log(|_l| true, Some(num));
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

#[derive(Clone, Debug, VmType, Pushable, Getable)]
#[cfg_attr(feature = "mongo", derive(Deserialize))]
pub struct Object {
    #[cfg_attr(feature = "mongo", serde(rename(deserialize = "_id")))]
    pub id: u32,
    pub name: String,
    pub typ: String,
    #[cfg_attr(feature = "mongo", serde(default))]
    pub desc: String,
    #[cfg_attr(feature = "mongo", serde(default))]
    pub attrs: Attrs,
}

impl Object {
    fn new(name: String, typ: String, desc: String) -> StorageResult<Object> {
        let mut storage = lock_store()?;
        let id = storage.create_obj(&name, &typ)?;
        Ok(Object {
            id,
            name,
            typ,
            desc,
            attrs: BTreeMap::new(),
        })
    }

    fn get(id: u32) -> StorageResult<Object> {
        lock_store()?.get_obj(id)
    }

    fn set_desc(obj: Object, desc: &str) -> StorageResult<()> {
        lock_store()?.obj_set_desc(obj.id, desc.to_string())
    }

    fn set_attr(obj: Object, key: &str, val: AttrValue) -> StorageResult<()> {
        lock_store()?.obj_set_attr(obj.id, key.to_string(), val)
    }

    fn del_attr(obj: Object, attr: &str) -> StorageResult<()> {
        lock_store()?.obj_del_attr(obj.id, attr)
    }

    fn find(filter: FunctionRef<fn(Object) -> bool>, limit: Option<usize>) -> StorageResult<Vec<Object>> {
        Ok(lock_store()?.find_obj(|o| filter.clone().call(o.clone()).unwrap(), limit))
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
                type Log => Log,
                new => primitive!(2, Log::new),
                get => primitive!(1, Log::get),
                set_attr => primitive!(3, Log::set_attr),
                find => primitive!(2, Log::find),
                list => primitive!(1, Log::list),
            },

            obj => record! {
                type Object => Object,
                new => primitive!(3, Object::new),
                get => primitive!(1, Object::get),
                set_desc => primitive!(2, Object::set_desc),
                set_attr => primitive!(3, Object::set_attr),
                del_attr => primitive!(2, Object::del_attr),
                find => primitive!(2, Object::find),
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
                lock_store()?.add_gluon(pat, func)
            }),
            repeat => primitive!(3, |start, every, stop| {
                Repeated::new(start, every, stop)
            }),
        },
    )
}
