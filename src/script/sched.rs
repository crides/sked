use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bson::{Bson, Document};
use gluon::{vm::ExternModule, Thread};
use lazy_static::lazy_static;

use crate::storage::{Error, Log, Object, Result as StorageResult, Storage};

lazy_static! {
    pub static ref STATE: APIState = APIState(Arc::new(Mutex::new(Storage::new())));
}

pub struct APIState(Arc<Mutex<Storage>>);

fn log_new(typ: String, map: BTreeMap<String, String>) -> StorageResult<Log> {
    let attrs = map.into_iter().map(|(k, v)| (k, Bson::String(v))).collect::<Document>();
    let id = STATE.0.lock().unwrap().create_log(&typ, attrs)?;
    // FIXME optimize this
    STATE.0.lock().unwrap().get_log(id)
}

fn log_get(id: i32) -> StorageResult<Log> {
    STATE.0.lock().unwrap().get_log(id)
}

fn obj_get(id: i32) -> StorageResult<Object> {
    STATE.0.lock().unwrap().get_obj(id)
}

fn log_set_attr(log: Log, key: &str, val: &str) -> StorageResult<()> {
    STATE.0.lock().unwrap().log_set_attr(log.id, key, val)
}

fn obj_new(name: String, typ: String, map: BTreeMap<String, String>) -> StorageResult<Object> {
    let mut storage = STATE.0.lock().unwrap();
    let id = storage.create_obj(&name, &typ)?;
    if map.contains_key("desc") {
        storage.obj_set_desc(id, map.get("desc").unwrap())?;
    }
    // FIXME optimize this
    STATE.0.lock().unwrap().get_obj(id)
}

fn obj_set_desc(obj: Object, desc: &str) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_set_desc(obj.id, desc)
}

fn obj_add_sub(obj: Object, sub: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_add_sub(obj.id, sub)
}

fn obj_add_ref(obj: Object, rf: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_add_ref(obj.id, rf)
}

fn obj_add_dep(obj: Object, dep: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_add_dep(obj.id, dep)
}

fn obj_set_attr(obj: Object, key: &str, val: &str) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_set_attr(obj.id, key, val)
}

fn obj_del_sub(obj: Object, sub: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_del_sub(obj.id, sub)
}

fn obj_del_ref(obj: Object, rf: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_del_ref(obj.id, rf)
}

fn obj_del_dep(obj: Object, dep: i32) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_del_dep(obj.id, dep)
}

fn obj_del_attr(obj: Object, attr: &str) -> StorageResult<()> {
    STATE.0.lock().unwrap().obj_del_attr(obj.id, attr)
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    thread.register_type::<Error>("sched.Error", &[])?;
    ExternModule::new(
        thread,
        record! {
            log => record! {
                type Log => Log,
                new => primitive!(2, log_new),
                get => primitive!(1, log_get),
                set_attr => primitive!(3, log_set_attr),
            },

            obj => record! {
                type Object => Object,
                new => primitive!(3, obj_new),
                get => primitive!(1, obj_get),
                set_desc => primitive!(2, obj_set_desc),
                add_sub => primitive!(2, obj_add_sub),
                add_dep => primitive!(2, obj_add_dep),
                add_ref => primitive!(2, obj_add_ref),
                set_attr => primitive!(3, obj_set_attr),
                del_sub => primitive!(2, obj_del_sub),
                del_ref => primitive!(2, obj_del_ref),
                del_dep => primitive!(2, obj_del_dep),
                del_attr => primitive!(2, obj_del_attr),
            },

            add_handler => primitive!(2, |pat, func| {
                STATE.0.lock().unwrap().add_gluon(pat, func)
            }),
        },
    )
}
