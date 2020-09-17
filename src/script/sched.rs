use std::collections::BTreeMap;
use std::sync::Mutex;

use gluon::{vm::ExternModule, Thread};
use lazy_static::lazy_static;

use crate::storage::{Error, Result as StorageResult, Storage};
use crate::script::{
    time::DateTime, task::{Task, Event, Weekday, Month, Every, Stop}
};

lazy_static! {
    pub static ref STORE: Mutex<Storage> = Mutex::new(Storage::new());
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable, Serialize, Deserialize)]
#[gluon_trace(skip)]
#[serde(untagged)]
pub enum AttrValue {
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable, Serialize, Deserialize)]
#[gluon_trace(skip)]
pub struct Attr(pub BTreeMap<String, AttrValue>);

impl Default for Attr {
    fn default() -> Self {
        Attr(BTreeMap::default())
    }
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable, Deserialize)]
#[gluon_trace(skip)]
pub struct Log {
    #[serde(rename(deserialize = "_id"))]
    pub id: i32,
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    pub time: DateTime,
    #[serde(default)]
    pub attrs: Attr,
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable, Deserialize)]
#[gluon_trace(skip)]
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

impl Log {
    fn new(typ: String, attrs: Attr) -> StorageResult<Log> {
        STORE.lock().unwrap().create_log_attrs(&typ, attrs)
    }

    fn get(id: i32) -> StorageResult<Log> {
        STORE.lock().unwrap().get_log(id)
    }

    fn set_attr(self, key: &str, val: AttrValue) -> StorageResult<()> {
        STORE.lock().unwrap().log_set_attr(self.id, key, val)
    }
}

impl Object {
    fn new(name: String, typ: String, map: BTreeMap<String, String>) -> StorageResult<Object> {
        let mut storage = STORE.lock().unwrap();
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
        if map.contains_key("desc") {
            let desc = map.get("desc").unwrap();
            storage.obj_set_desc(id, desc)?;
            obj.desc = Some(desc.into());
        }
        Ok(obj)
    }

    fn get(id: i32) -> StorageResult<Object> {
        STORE.lock().unwrap().get_obj(id)
    }

    fn set_desc(obj: Object, desc: &str) -> StorageResult<()> {
        STORE.lock().unwrap().obj_set_desc(obj.id, desc)
    }

    fn add_sub(obj: Object, sub: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_add_sub(obj.id, sub)
    }

    fn add_ref(obj: Object, rf: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_add_ref(obj.id, rf)
    }

    fn add_dep(obj: Object, dep: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_add_dep(obj.id, dep)
    }

    fn set_attr(obj: Object, key: &str, val: AttrValue) -> StorageResult<()> {
        STORE.lock().unwrap().obj_set_attr(obj.id, key, val)
    }

    fn del_sub(obj: Object, sub: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_del_sub(obj.id, sub)
    }

    fn del_ref(obj: Object, rf: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_del_ref(obj.id, rf)
    }

    fn del_dep(obj: Object, dep: i32) -> StorageResult<()> {
        STORE.lock().unwrap().obj_del_dep(obj.id, dep)
    }

    fn del_attr(obj: Object, attr: &str) -> StorageResult<()> {
        STORE.lock().unwrap().obj_del_attr(obj.id, attr)
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

            add_handler => primitive!(2, |pat, func| {
                STORE.lock().unwrap().add_gluon(pat, func)
            }),
        },
    )
}
