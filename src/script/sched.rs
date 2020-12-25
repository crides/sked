use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, TryLockError};

use gluon::{
    vm::{api::FunctionRef, ExternModule},
    Thread,
};
use lazy_static::lazy_static;
pub use serde_json::Value as AttrValue;

use crate::{
    script::{
        task::{Event, Task},
        time::DateTime,
    },
    storage::{Error, Every, Result as StorageResult, Stop, Storage, Repeated},
};

lazy_static! {
    pub static ref STORE: Mutex<Storage> = Mutex::new(Storage::new());
}

pub type Attrs = BTreeMap<String, AttrValue>;

#[derive(Clone, Debug, VmType, Pushable, Getable)]
#[cfg_attr(feature = "mongo", derive(Deserialize))]
pub struct Log {
    #[cfg_attr(feature = "mongo", serde(rename(deserialize = "_id")))]
    pub id: i32,
    pub typ: String,
    pub time: DateTime,
    #[cfg_attr(feature = "mongo", serde(default))]
    pub attrs: Attrs,
}

pub fn lock_store() -> StorageResult<MutexGuard<'static, Storage>> {
    match STORE.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(Error::Deadlock),
        Err(TryLockError::Poisoned(_)) => panic!("STORE lock poisoned"),
    }
}

impl Log {
    fn new(typ: String, attrs: Attrs) -> StorageResult<Log> {
        lock_store()?.create_log(typ, attrs)
    }

    fn get(id: i32) -> StorageResult<Log> {
        lock_store()?.get_log(id)
    }

    fn set_attr(self, key: String, val: AttrValue) -> StorageResult<()> {
        lock_store()?.log_set_attr(self.id, key, val)
    }

    fn find(filter: FunctionRef<fn(Log) -> bool>, limit: Option<usize>) -> Vec<Log> {
        lock_store()
            .unwrap()
            .find_log(|l| filter.clone().call(l.clone()).unwrap(), limit)
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable)]
#[cfg_attr(feature = "mongo", derive(Deserialize))]
pub struct Object {
    #[cfg_attr(feature = "mongo", serde(rename(deserialize = "_id")))]
    pub id: i32,
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

    fn get(id: i32) -> StorageResult<Object> {
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

    fn find(filter: FunctionRef<fn(Object) -> bool>, limit: Option<usize>) -> Vec<Object> {
        lock_store()
            .unwrap()
            .find_obj(|o| filter.clone().call(o.clone()).unwrap(), limit)
    }
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            type Every => Every,
            type Stop => Stop,
            type Error => Error,
            log => record! {
                type Log => Log,
                new => primitive!(2, Log::new),
                get => primitive!(1, Log::get),
                set_attr => primitive!(3, Log::set_attr),
                find => primitive!(2, Log::find),
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
