use std::borrow::Cow;
use std::convert::TryInto;
use std::sync::Mutex;

use chrono::{TimeZone, Utc};
use serde::de::{Deserialize, DeserializeOwned};
use serde_json::json;
use sled::{Db, Tree};

use crate::{
    attrs,
    handler::{LogHandler, LogHandlers},
    storage::time::{DateTime, Duration},
    storage::{api::*, Error, Error as StorageError, OptRepeated, Result as StorageResult},
};

pub struct Storage {
    db: Db,
    meta: Tree,
    logs: Tree,
    objs: Tree,
    handlers: Mutex<LogHandlers>,
}

fn ser_obj<S: Into<impl serde::Serialize> + ApiObj>(obj: S) -> Vec<u8> {
    serde_json::to_vec(&obj.into()).unwrap()
}

fn ser_log<S: Into<impl serde::Serialize> + ApiLog>(log: S) -> Vec<u8> {
    serde_json::to_vec(&log.into()).unwrap()
}

fn ser<S: ?Sized + serde::Serialize>(obj: &S) -> Vec<u8> {
    serde_json::to_vec(obj).unwrap()
}

fn deser<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> T {
    serde_json::from_slice(bytes).unwrap()
}

fn deser_obj<'de, T: ApiObj + Deserialize<'de>>(bytes: &'de [u8]) -> StorageResult<RawObj<T>> {
    let proto: ProtoObj = serde_json::from_slice(bytes).map_err(Error::Serde)?;
    let deser_typ = proto.typ;
    if deser_typ != T::OBJ_TYPE {
        return Err(Error::TypeMismatch {
            expected: T::OBJ_TYPE.into(),
            actual: deser_typ.into(),
        });
    }
    serde_json::from_slice(bytes).map_err(Error::Serde)
}

fn deser_log<'de, T: ApiLog + Deserialize<'de>>(bytes: &'de [u8]) -> StorageResult<RawLog<T>> {
    let proto: ProtoLog = serde_json::from_slice(bytes).map_err(Error::Serde)?;
    let deser_typ = proto.typ;
    if deser_typ != T::LOG_TYPE {
        return Err(Error::TypeMismatch {
            expected: T::LOG_TYPE.into(),
            actual: deser_typ.into(),
        });
    }
    serde_json::from_slice(bytes).map_err(Error::Serde)
}

fn try_deser<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> serde_json::Result<T> {
    serde_json::from_slice(bytes)
}

fn ser_obj_id(id: ObjId) -> Vec<u8> {
    id.0.to_be_bytes().to_vec()
}

fn ser_log_id(id: LogId) -> Vec<u8> {
    id.0.to_be_bytes().to_vec()
}

fn deser_log_id(bytes: &[u8]) -> LogId {
    LogId(u32::from_be_bytes(bytes.try_into().expect("malformed log id in db")))
}

fn deser_obj_id(bytes: &[u8]) -> ObjId {
    ObjId(u32::from_be_bytes(bytes.try_into().expect("malformed obj id in db")))
}

// We don't need meta cuz the ids are just the lengths of the arrays
#[derive(Debug, Serialize, Deserialize)]
struct DbData {
    logs: Vec<serde_json::Value>,
    objs: Vec<serde_json::Value>,
}

impl Storage {
    pub fn new() -> Storage {
        let config_dir = dirs::config_dir().unwrap().join("sched"); // FIXME
        let db = sled::open(config_dir.join("sched.db")).unwrap();
        let meta = db.open_tree("meta").unwrap();
        if !meta.contains_key("logs_id").unwrap() {
            meta.insert("logs_id", ser_log_id(LogId(1))).unwrap();
        }
        if !meta.contains_key("objs_id").unwrap() {
            meta.insert("objs_id", ser_obj_id(ObjId(1))).unwrap();
        }
        let objs = db.open_tree("objs").unwrap();
        let has_valid_state = objs
            .get(&ser_obj_id(State::ID))
            .unwrap()
            .map(|o| {
                let state: Result<RawObj<State>, _> = deser_obj(&o);
                state.is_ok()
            })
            .unwrap_or(false);
        if !has_valid_state {
            objs.insert(ser_obj_id(State::ID), ser(&State::new())).unwrap();
        }
        Storage {
            logs: db.open_tree("logs").unwrap(),
            db,
            meta,
            objs,
            handlers: Mutex::new(LogHandlers::new()),
        }
    }

    #[cfg(features = "scripting")]
    pub fn add_gluon<T: ApiLog>(&self, pat: &str, f: LogHandler) -> StorageResult<()> {
        self.handlers.lock().unwrap().add_gluon(pat, f)
    }

    fn get_log_id(&self) -> LogId {
        deser_log_id(
            &self
                .meta
                .fetch_and_update("logs_id", |old| {
                    Some(ser_log_id(LogId(deser_log_id(old.unwrap()).0 + 1)))
                })
                .unwrap()
                .unwrap(),
        )
    }

    fn append_log_raw<L: ApiLog>(&self, log: L, attrs: Option<Attrs>) -> StorageResult<LogId> {
        let id = self.get_log_id();
        let raw = RawLog {
            attrs,
            time: DateTime::now(),
            typ: L::LOG_TYPE.into(),
            inner: log,
        };
        let serialized = ser(&raw);
        let proto: ProtoLog = serde_json::from_slice(&serialized).unwrap();
        self.logs.insert(ser_log_id(id), serialized).unwrap();
        let log = proto.with_id(id);
        self.handlers.lock().unwrap().handle(&log);
        Ok(id)
    }

    pub fn append_log<L: ApiLog>(&self, log: L) -> StorageResult<LogId> {
        Storage::append_log_raw(&self, log, None)
    }

    pub fn append_log_attr<L: ApiLog>(&self, log: L, attrs: Attrs) -> StorageResult<LogId> {
        Storage::append_log_raw(&self, log, Some(attrs))
    }

    pub fn get_log<L: ApiLog + DeserializeOwned>(&self, id: LogId) -> StorageResult<Log<L>> {
        self.logs
            .get(ser_log_id(id))
            .unwrap()
            .map(|l| deser_log(&l).map(|r| r.with_id(id)))
            .unwrap_or(Err(Error::InvalidLogID(id)))
    }

    fn filter_log_by<F: Fn(&ScriptLog) -> bool>(
        iter: impl Iterator<Item = sled::Result<(sled::IVec, sled::IVec)>>,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptLog> {
        let iter = iter
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<ProtoLog>(&v).with_id(deser_log_id(&k)))
            .filter(filter);
        if let Some(limit) = limit {
            iter.take(limit).collect()
        } else {
            iter.collect()
        }
    }

    pub fn find_log<F: Fn(&ScriptLog) -> bool>(&self, filter: F, limit: Option<usize>) -> Vec<ScriptLog> {
        Storage::filter_log_by(self.logs.iter().rev(), filter, limit)
    }

    pub fn find_log_old<F: Fn(&ScriptLog) -> bool>(&self, filter: F, limit: Option<usize>) -> Vec<ScriptLog> {
        Storage::filter_log_by(self.logs.iter(), filter, limit)
    }

    pub fn find_log_from<F: Fn(&ScriptLog) -> bool>(
        &self,
        id: LogId,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptLog> {
        Storage::filter_log_by(self.logs.range(..ser_log_id(id)).rev(), filter, limit)
    }

    pub fn find_log_old_from<F: Fn(&ScriptLog) -> bool>(
        &self,
        id: LogId,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptLog> {
        Storage::filter_log_by(self.logs.range(ser_log_id(id)..), filter, limit)
    }

    // Object stuff
    fn get_obj_id(&self) -> ObjId {
        deser_obj_id(
            &self
                .meta
                .fetch_and_update("objs_id", |old| {
                    Some(ser_obj_id(ObjId(deser_obj_id(old.unwrap()).0 + 1)))
                })
                .unwrap()
                .unwrap(),
        )
    }

    pub fn get_state(&self) -> StorageResult<State> {
        Ok(self.get_obj(State::ID)?.inner)
    }

    pub fn set_state(&self, state: State) -> StorageResult<()> {
        // TODO diff state and log?
        self.set_obj(State::ID, state)
    }

    // pub fn state_get(&self, attr: &str) -> Option<AttrValue> {
    //     let state: Option<Obj<State>> = self.get_obj(State::ID).ok();
    //     state.map(|s| s.attrs.get(attr).cloned()).flatten()
    // }

    // pub fn state_set(&self, attr: &str, val: AttrValue) {
    //     self.obj_set_attr(0, attr.to_owned(), val).unwrap();
    // }

    fn create_obj_with_id<O: ApiObj>(
        &self,
        id: ObjId,
        obj: O,
        name: String,
        desc: Option<String>,
        attrs: Option<Attrs>,
    ) -> StorageResult<ObjId> {
        let obj = RawObj {
            inner: obj,
            name,
            typ: O::OBJ_TYPE.into(),
            desc,
            attrs,
        };
        self.objs.insert(ser_obj_id(id), ser(&obj)).unwrap();
        self.append_log(CreateObj {
            id,
            typ: O::OBJ_TYPE.into(),
        })?;
        Ok(id)
    }

    pub fn create_obj<O: ApiObj>(
        &self,
        obj: O,
        name: String,
        desc: Option<String>,
        attrs: Option<Attrs>,
    ) -> StorageResult<ObjId> {
        let id = self.get_obj_id();
        self.create_obj_with_id(id, obj, name, desc, attrs)
    }

    pub fn create_obj_with<O: ApiObj>(
        &self,
        name: String,
        desc: Option<String>,
        attrs: Option<Attrs>,
        f: impl FnOnce(ObjId) -> StorageResult<O>,
    ) -> StorageResult<ObjId> {
        let id = self.get_obj_id();
        let obj = f(id)?;
        self.create_obj_with_id(id, obj, name, desc, attrs)
    }

    pub fn obj_set_desc(&self, id: ObjId, desc: Option<String>) -> StorageResult<()> {
        let new_desc = desc.clone();
        let mut obj: ProtoObj = deser(&self.objs.get(ser_obj_id(id))?.ok_or(StorageError::InvalidObjID(id))?);
        if obj.desc.is_none() && desc.is_none() {
            // Simply skip cuz no change needs to be done
            return Ok(());
        }
        let old_desc = obj.desc.take();
        obj.desc = desc;
        self.objs.insert(ser_obj_id(id), ser(&obj))?;
        let diff = match (old_desc, new_desc) {
            (Some(o), Some(n)) => Diff::Diff(o, n),
            (None, Some(n)) => Diff::New(n),
            (Some(o), None) => Diff::Del(o),
            (None, None) => unreachable!(),
        };
        self.append_log(ObjSetDesc { id, diff })?;
        Ok(())
    }

    fn obj_set_attr_raw(&self, id: ObjId, attr: String, val: Option<AttrValue>) -> StorageResult<()> {
        let new_val = val.clone();
        let mut obj: ProtoObj = deser(&self.objs.get(ser_obj_id(id))?.ok_or(StorageError::InvalidObjID(id))?);
        let old_val = if let Some(ref mut attrs) = obj.attrs {
            if !attrs.contains_key(&attr) && val.is_none() {
                return Err(StorageError::DelNonExistent(id, attr));
            }
            match val {
                Some(val) => attrs.insert(attr.clone(), val),
                None => attrs.remove(&attr),
            }
        } else {
            None
        };
        self.objs.insert(ser_obj_id(id), ser(&obj))?;
        let diff = match (old_val, new_val) {
            (Some(o), Some(n)) => Diff::Diff(o, n),
            (None, Some(n)) => Diff::New(n),
            (Some(o), None) => Diff::Del(o),
            (None, None) => unreachable!(),
        };
        self.append_log(ObjSetAttr { id, attr, diff })?;
        Ok(())
    }

    pub fn obj_set_attr(&self, id: ObjId, attr: String, val: AttrValue) -> StorageResult<()> {
        self.obj_set_attr_raw(id, attr, Some(val))
    }

    pub fn obj_set_attrs(&self, id: ObjId, attrs: Attrs) -> StorageResult<()> {
        attrs
            .into_iter()
            .map(|(key, val)| self.obj_set_attr(id, key, val))
            .collect()
    }

    pub fn obj_del_attr(&self, id: ObjId, attr: String) -> StorageResult<()> {
        self.obj_set_attr_raw(id, attr, None)
    }

    pub fn get_obj<O: ApiObj + DeserializeOwned>(&self, id: ObjId) -> StorageResult<Obj<O>> {
        self.objs
            .get(ser_obj_id(id))
            .unwrap()
            .map(|o| deser_obj(&o).map(|r| r.with_id(id)))
            .unwrap_or(Err(Error::InvalidObjID(id)))
    }

    pub fn set_obj<O: ApiObj>(&self, id: ObjId, obj: O) -> StorageResult<()> {
        // TODO diff props & attrs here?
        self.objs.insert(ser_obj_id(id), ser(&obj))?;
        Ok(())
    }

    fn filter_script_obj_by<F: Fn(&ScriptObj) -> bool>(
        iter: impl Iterator<Item = sled::Result<(sled::IVec, sled::IVec)>>,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptObj> {
        let iter = iter
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<ProtoObj>(&v).with_id(deser_obj_id(&k)))
            .filter(filter);
        if let Some(limit) = limit {
            iter.take(limit).collect()
        } else {
            iter.collect()
        }
    }

    fn filter_obj_by<O: ApiObj + DeserializeOwned, F: Fn(&Obj<O>) -> bool>(
        iter: impl Iterator<Item = sled::Result<(sled::IVec, sled::IVec)>>,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<Obj<O>> {
        let iter = iter
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<RawObj<O>>(&v).with_id(deser_obj_id(&k)))
            .filter(filter);
        if let Some(limit) = limit {
            iter.take(limit).collect()
        } else {
            iter.collect()
        }
    }

    pub fn find_obj<O: ApiObj + DeserializeOwned, F: Fn(&Obj<O>) -> bool>(
        &self,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<Obj<O>> {
        Storage::filter_obj_by(self.objs.iter().rev(), filter, limit)
    }

    pub fn script_find_obj<F: Fn(&ScriptObj) -> bool>(&self, filter: F, limit: Option<usize>) -> Vec<ScriptObj> {
        Storage::filter_script_obj_by(self.objs.iter().rev(), filter, limit)
    }

    pub fn script_find_obj_old<F: Fn(&ScriptObj) -> bool>(&self, filter: F, limit: Option<usize>) -> Vec<ScriptObj> {
        Storage::filter_script_obj_by(self.objs.iter(), filter, limit)
    }

    pub fn script_find_obj_from<F: Fn(&ScriptObj) -> bool>(
        &self,
        id: ObjId,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptObj> {
        Storage::filter_script_obj_by(self.objs.range(..ser_obj_id(id)).rev(), filter, limit)
    }

    pub fn script_find_obj_old_from<F: Fn(&ScriptObj) -> bool>(
        &self,
        id: ObjId,
        filter: F,
        limit: Option<usize>,
    ) -> Vec<ScriptObj> {
        Storage::filter_script_obj_by(self.objs.range(ser_obj_id(id)..), filter, limit)
    }

    pub fn create_task(
        &self,
        name: String,
        desc: Option<String>,
        attrs: Option<Attrs>,
        deadline: OptRepeated,
        priority: u32,
    ) -> StorageResult<ObjId> {
        self.create_obj_with(name, desc, attrs, |id| {
            // FIXME use batch (atomic) or transaction sematics
            // TODO inherit notification from config 
            let mut task = Task::new(deadline, priority, Vec::new());
            match task.deadline {
                OptRepeated::Single(time) => {
                    dbg!(&time);
                    let new_id = self.new_sub_task(id, time, None)?;
                    task.cache.push(new_id);
                }
                OptRepeated::Repeat(ref mut repeat) => {
                    // FIXME attribute casting should be an system error and should create log entry
                    for _ in 0..task.gen_ahead {
                        if let Some(next_time) = repeat.next() {
                            let new_id = self.new_sub_task(id, next_time, None)?;
                            task.cache.push(new_id);
                        } else {
                            break;
                        }
                    }
                }
            }
            Ok(task)
        })
    }

    fn new_sub_task(&self, id: ObjId, deadline: DateTime, notifications: Option<&Vec<Duration>>) -> StorageResult<ObjId> {
        self.create_obj_with("subtask".into(), None, None, |_| {
            Ok(SubTask::new(id, deadline, notifications.cloned().unwrap_or_default()))
        })
    }

    // FIXME Error on finished tasks? Or how to handle collision
    /// id is the id for the sub task
    pub fn task_finish(&self, id: ObjId, finished: DateTime) -> StorageResult<()> {
        let mut sub: SubTask = self.get_obj(id)?.inner;
        let mut task: Task = self.get_obj(sub.task_id)?.inner;
        if let OptRepeated::Repeat(ref mut repeat) = task.deadline {
            let cache_size = task.cache_size + task.gen_ahead + 1;
            if let Some(next_time) = repeat.next() {
                let new_id = self.new_sub_task(id, next_time, None)?;
                // We only generate one cuz there can be only 1 task completed
                task.cache.push(new_id);
                if task.cache.len() > cache_size as usize {
                    task.cache.remove(0);
                }
            }
            sub.finished = Some(finished);
            self.set_obj(sub.task_id, task)?;
            self.set_obj(id, sub)?;
        }
        // FIXME missing logs on props & attrs setting
        self.append_log(TaskFinish { id })?;
        Ok(())
    }

    pub fn find_current(&self, id: ObjId) -> StorageResult<Option<ObjId>> {
        // It should
        let current_utc = Utc::now();
        let task: Task = self.get_obj(id)?.inner;
        let balanced = task.flavor == TaskFlavor::Balanced;
        let sub_tasks: Vec<Obj<SubTask>> = task
            .cache
            .iter()
            .map(|&i| self.get_obj(i))
            .collect::<Result<_, _>>()?;
        let unfinished = sub_tasks.into_iter()
            .filter(|o| o.inner.finished.is_none())
            .collect::<Vec<_>>();

        let deadlines = unfinished
            .iter()
            .map(|sub| sub.inner.deadline)
            .collect::<Vec<_>>();
        let len = unfinished.len();
        let grace = chrono::Duration::minutes(5);
        // TODO sort this instead so that past unfinished tasks maybe current?
        for i in 0..len {
            let deadline = deadlines[i];
            let criterion = if balanced {
                i == len - 1 || (deadline.0 + (deadlines[i + 1].0 - deadline.0) / 2) > current_utc
            } else {
                current_utc < deadline.0 + grace
            };
            if criterion {
                return Ok(Some(unfinished[i].id));
            }
        }
        Ok(None)
    }

    pub fn create_event(
        &self,
        name: String,
        start: OptRepeated,
        duration: Duration,
        desc: Option<String>,
        attrs: Option<Attrs>,
    ) -> StorageResult<ObjId> {
        let event = Event {
            start,
            duration,
        };
        let id = self.create_obj(event, name, desc, attrs)?;
        Ok(id)
    }

    pub fn export(&self) -> serde_json::Value {
        let logs = self
            .logs
            .iter()
            .map(|r| r.unwrap())
            .enumerate()
            .map(|(i, (k, v))| {
                assert_eq!(i as u32 + 1, deser_log_id(&k).0);
                deser(&v)
            })
            .collect();
        let objs = self
            .objs
            .iter()
            .map(|r| r.unwrap())
            .enumerate()
            .map(|(_i, (_k, v))| {
                // assert_eq!(i as u32, deser_id(&k));
                deser(&v)
            })
            .collect();
        serde_json::to_value(DbData { logs, objs }).unwrap()
    }

    pub fn import(&self, s: &str) {
        let data: DbData = serde_json::from_str(s).unwrap();
        self.meta.clear().unwrap();
        self.logs.clear().unwrap();
        self.objs.clear().unwrap();
        for (i, log) in data.logs.iter().enumerate() {
            self.logs.insert(ser_log_id(LogId(i as u32 + 1)), ser(log)).unwrap();
        }
        for (i, obj) in data.objs.iter().enumerate() {
            self.objs.insert(ser_obj_id(ObjId(i as u32)), ser(obj)).unwrap();
        }
        self.meta
            .insert("logs_id", ser_log_id(LogId(data.logs.len() as u32 + 1)))
            .unwrap();
        self.meta
            .insert("objs_id", ser_obj_id(ObjId(data.objs.len() as u32 + 1)))
            .unwrap();
        self.db.flush().unwrap();
    }
}
