use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, TryLockError};

use bson::to_document;
use gluon::{vm::ExternModule, Thread};
use lazy_static::lazy_static;

use crate::script::{
    task::{Event, Every, Month, Stop, Task, Weekday},
    time::DateTime,
};
use crate::storage::{Error, Result as StorageResult, Storage};

lazy_static! {
    pub static ref STORE: Mutex<Storage> = Mutex::new(Storage::new());
}

#[derive(Clone, Debug, VmType, Pushable, Getable, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttrValue {
    Int(i64),
    Float(f64),
    String(String),
}

impl AttrValue {
    fn show(self) -> String {
        match self {
            AttrValue::Int(i) => i.to_string(),
            AttrValue::Float(f) => f.to_string(),
            AttrValue::String(s) => format!("'{}'", s),
        }
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable, Serialize, Deserialize)]
pub struct Attr(pub BTreeMap<String, AttrValue>);

impl Default for Attr {
    fn default() -> Self {
        Attr(BTreeMap::default())
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable, Deserialize)]
pub struct Log {
    #[serde(rename(deserialize = "_id"))]
    pub id: i32,
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    pub time: DateTime,
    #[serde(default)]
    pub attrs: Attr,
}

pub fn lock_store() -> StorageResult<MutexGuard<Storage>> {
    match STORE.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(Error::Deadlock),
        Err(TryLockError::Poisoned(_)) => panic!("STORE lock poisoned"),
    }
}

impl Log {
    fn new(typ: String, attrs: Attr) -> StorageResult<Log> {
        lock_store()?.create_log_attrs(&typ, attrs)
    }

    fn get(id: i32) -> StorageResult<Log> {
        lock_store()?.get_log(id)
    }

    fn set_attr(self, key: &str, val: AttrValue) -> StorageResult<()> {
        lock_store()?.log_set_attr(self.id, key, val)
    }

    fn find(filter: Attr, limit: Option<usize>) -> Vec<Log> {
        lock_store()?.find_log(to_document(&filter).unwrap(), limit)
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable, Deserialize)]
pub struct Object {
    #[serde(rename(deserialize = "_id"))]
    pub id: i32,
    pub name: String,
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub deps: Vec<i32>,
    #[serde(default)]
    pub subs: Vec<i32>,
    #[serde(default)]
    pub refs: Vec<i32>,
    #[serde(default)]
    pub attrs: Attr,
}

impl Object {
    fn new(name: String, typ: String, desc: Option<String>) -> StorageResult<Object> {
        let mut storage = lock_store()?;
        let id = storage.create_obj(&name, &typ)?;
        let mut obj = Object {
            id,
            name,
            typ,
            desc: None,
            deps: Vec::new(),
            subs: Vec::new(),
            refs: Vec::new(),
            attrs: Attr(BTreeMap::new()),
        };
        if let Some(desc) = desc {
            storage.obj_set_desc(id, &desc)?;
            obj.desc = Some(desc);
        }
        Ok(obj)
    }

    fn get(id: i32) -> StorageResult<Object> {
        lock_store()?.get_obj(id)
    }

    fn set_desc(obj: Object, desc: &str) -> StorageResult<()> {
        lock_store()?.obj_set_desc(obj.id, desc)
    }

    fn add_sub(obj: Object, sub: i32) -> StorageResult<()> {
        lock_store()?.obj_add_sub(obj.id, sub)
    }

    fn add_ref(obj: Object, rf: i32) -> StorageResult<()> {
        lock_store()?.obj_add_ref(obj.id, rf)
    }

    fn add_dep(obj: Object, dep: i32) -> StorageResult<()> {
        lock_store()?.obj_add_dep(obj.id, dep)
    }

    fn set_attr(obj: Object, key: &str, val: AttrValue) -> StorageResult<()> {
        lock_store()?.obj_set_attr(obj.id, key, val)
    }

    fn del_sub(obj: Object, sub: i32) -> StorageResult<()> {
        lock_store()?.obj_del_sub(obj.id, sub)
    }

    fn del_ref(obj: Object, rf: i32) -> StorageResult<()> {
        lock_store()?.obj_del_ref(obj.id, rf)
    }

    fn del_dep(obj: Object, dep: i32) -> StorageResult<()> {
        lock_store()?.obj_del_dep(obj.id, dep)
    }

    fn del_attr(obj: Object, attr: &str) -> StorageResult<()> {
        lock_store()?.obj_del_attr(obj.id, attr)
    }

    fn find(filter: Attr, limit: Option<usize>) -> Vec<Object> {
        lock_store()?.find_obj(to_document(&filter).unwrap(), limit)
    }
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            type Weekday => Weekday,
            type Month => Month,
            type Every => Every,
            type Stop => Stop,
            type Error => Error,
            type AttrValue => AttrValue,
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
                add_sub => primitive!(2, Object::add_sub),
                add_dep => primitive!(2, Object::add_dep),
                add_ref => primitive!(2, Object::add_ref),
                set_attr => primitive!(3, Object::set_attr),
                del_sub => primitive!(2, Object::del_sub),
                del_ref => primitive!(2, Object::del_ref),
                del_dep => primitive!(2, Object::del_dep),
                del_attr => primitive!(2, Object::del_attr),
                find => primitive!(2, Object::find),
            },

            task => record! {
                type Task => Task,
                new => primitive!(5, Task::new),
                get => primitive!(1, Task::get),
            },

            event => record! {
                type Event => Event,
                new => primitive!(6, Event::new),
                get => primitive!(1, Event::get),
            },

            attr => record! {
                show => primitive!(1, AttrValue::show),
            },

            handle => primitive!(2, |pat, func| {
                lock_store()?.add_gluon(pat, func)
            }),
        },
    )
}
