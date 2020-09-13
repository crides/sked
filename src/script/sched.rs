use std::collections::BTreeMap;
use std::sync::Mutex;

use gluon::{vm::ExternModule, Thread};
use lazy_static::lazy_static;

use crate::storage::{Attr, AttrValue, Error, Log, Object, Result as StorageResult, Storage};

lazy_static! {
    static ref STORE: Mutex<Storage> = Mutex::new(Storage::new());
}

fn log_new(typ: String, attrs: Attr) -> StorageResult<Log> {
    STORE.lock().unwrap().create_log_attrs(&typ, attrs)
}

fn log_get(id: i32) -> StorageResult<Log> {
    STORE.lock().unwrap().get_log(id)
}

fn obj_get(id: i32) -> StorageResult<Object> {
    STORE.lock().unwrap().get_obj(id)
}

fn log_set_attr(log: Log, key: &str, val: AttrValue) -> StorageResult<()> {
    STORE.lock().unwrap().log_set_attr(log.id, key, val)
}

fn obj_new(name: String, typ: String, map: BTreeMap<String, String>) -> StorageResult<Object> {
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
        attrs: BTreeMap::new(),
    };
    if map.contains_key("desc") {
        let desc = map.get("desc").unwrap();
        storage.obj_set_desc(id, desc)?;
        obj.desc = Some(desc.into());
    }
    Ok(obj)
}

fn obj_set_desc(obj: Object, desc: &str) -> StorageResult<()> {
    STORE.lock().unwrap().obj_set_desc(obj.id, desc)
}

fn obj_add_sub(obj: Object, sub: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_add_sub(obj.id, sub)
}

fn obj_add_ref(obj: Object, rf: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_add_ref(obj.id, rf)
}

fn obj_add_dep(obj: Object, dep: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_add_dep(obj.id, dep)
}

fn obj_set_attr(obj: Object, key: &str, val: AttrValue) -> StorageResult<()> {
    STORE.lock().unwrap().obj_set_attr(obj.id, key, val)
}

fn obj_del_sub(obj: Object, sub: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_del_sub(obj.id, sub)
}

fn obj_del_ref(obj: Object, rf: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_del_ref(obj.id, rf)
}

fn obj_del_dep(obj: Object, dep: i32) -> StorageResult<()> {
    STORE.lock().unwrap().obj_del_dep(obj.id, dep)
}

fn obj_del_attr(obj: Object, attr: &str) -> StorageResult<()> {
    STORE.lock().unwrap().obj_del_attr(obj.id, attr)
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    thread.register_type::<AttrValue>("sched.AttrValue", &[])?;
    ExternModule::new(
        thread,
        record! {
            type Error => Error,
            type AttrValue => AttrValue,
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
                STORE.lock().unwrap().add_gluon(pat, func)
            }),
        },
    )
}
