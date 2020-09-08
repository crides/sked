use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, Mutex};

use bson::{Bson, Document};
use gluon::{
    import::add_extern_module, new_vm, vm::ExternModule, Result as GluonResult, RootedThread, Thread, ThreadExt,
};
use gluon_codegen::*;
use lazy_static::lazy_static;

use crate::storage::{Error, Log, Object, Result as StorageResult, Storage, GluonDateTime};

lazy_static! {
    pub static ref STATE: APIState = APIState(Arc::new(Mutex::new(Storage::new())));
}

#[derive(Clone, Debug, Trace, VmType, Userdata)]
#[gluon_userdata(clone)]
#[gluon_trace(skip)]
#[gluon(vm_type = "sched.LogRef")]
struct LogRef(i32);

#[derive(Clone, Debug, Trace, VmType, Userdata)]
#[gluon_userdata(clone)]
#[gluon_trace(skip)]
#[gluon(vm_type = "sched.ObjRef")]
struct ObjRef(i32);

pub struct APIState(Arc<Mutex<Storage>>);

fn load_sched_mod(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    thread.register_type::<LogRef>("sched.LogRef", &[])?;
    thread.register_type::<ObjRef>("sched.ObjRef", &[])?;
    thread.register_type::<Error>("sched.Error", &[])?;
    thread.register_type::<GluonDateTime>("sched.DateTime", &[])?;
    ExternModule::new(
        thread,
        record! {
            type Log => Log,
            type Object => Object,
            log => record! {
                type LogRef => LogRef,
                set_attr => primitive!(3, |rf: &LogRef, key: String, val: String| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .log_set_attr(rf.0, &key, &val)
                }),
                get => primitive!(1, |rf: &LogRef| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .get_log(rf.0)
                }),
            },

            obj => record! {
                type ObjRef => ObjRef,
                set_desc => primitive!(2, |rf: &ObjRef, desc: String| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_set_desc(rf.0, &desc)
                }),
                add_sub => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_add_sub(rf.0, obj)
                }),
                add_ref => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_add_ref(rf.0, obj)
                }),
                add_dep => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_add_dep(rf.0, obj)
                }),
                set_attr => primitive!(3, |rf: &ObjRef, key: String, val: String| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_set_attr(rf.0, &key, &val)
                }),
                del_sub => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_del_sub(rf.0, obj)
                }),
                del_ref => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_del_ref(rf.0, obj)
                }),
                del_dep => primitive!(2, |rf: &ObjRef, obj| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_del_dep(rf.0, obj)
                }),
                del_attr => primitive!(2, |rf: &ObjRef, key: String| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .obj_del_attr(rf.0, &key)
                }),
                get => primitive!(1, |rf: &ObjRef| {
                    STATE
                        .0
                        .lock()
                        .unwrap()
                        .get_obj(rf.0)
                }),
            },

            new_log => primitive!(2, |typ: String, map: BTreeMap<String, String>| -> StorageResult<LogRef> {
                let attrs = map
                    .into_iter()
                    .map(|(k, v)| (k, Bson::String(v)))
                    .collect::<Document>();
                let id = STATE
                    .0
                    .lock()
                    .unwrap()
                    .create_log(&typ, attrs)?;
                Ok(LogRef(id))
            }),
            get_log => primitive!(1, |id| LogRef(id)),
            new_obj => primitive!(3, |name: String, typ: String, map: BTreeMap<String, String>| -> StorageResult<ObjRef> {
                let mut storage = STATE.0.lock().unwrap();
                let id = storage.create_obj(&name, &typ)?;
                if map.contains_key("desc") {
                    storage.obj_set_desc(id, map.get("desc").unwrap())?;
                }
                Ok(ObjRef(id))
            }),
            get_obj => primitive!(1, |id| ObjRef(id)),
            add_handler => primitive!(2, |pat, func| {
                STATE.0.lock().unwrap().add_gluon(pat, func)
            }),
        },
    )
}

pub struct ScriptContext {
    pub vm: RootedThread,
}

impl ScriptContext {
    pub fn new() -> Self {
        let vm = new_vm();
        vm.run_io(true);
        add_extern_module(&vm, "sched", load_sched_mod);
        vm.load_file("std/map").unwrap();
        Self { vm }
    }

    pub fn init_user<P: AsRef<Path>>(&self, config_dir: P) -> GluonResult<()> {
        let config_dir = config_dir.as_ref();
        let init_file = config_dir.join("init.glu");
        let script = read_to_string(&init_file)?;
        self.vm.load_script(init_file.to_str().unwrap(), &script)?;
        Ok(())
    }
}
