use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::json;
use sled::Tree;

use crate::{
    script::{
        sched::{AttrValue, Attrs, Log, Object},
        task::{Event, Task},
        time::{DateTime, Duration},
    },
    signal::{SignalHandler, SignalHandlers},
    storage::{Error, Result, OptRepeated},
};

macro_rules! attrs {
    { $($tt:tt)+ } => {
        {
            use ::std::collections::BTreeMap;
            let mut object: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            serde_json::json_internal!(@object object () ($($tt)+) ($($tt)+));
            object
        }
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawLog {
    typ: String,
    time: DateTime,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    attrs: Attrs,
}

impl RawLog {
    fn with_id(self, id: i32) -> Log {
        Log {
            id,
            typ: self.typ,
            time: self.time,
            attrs: self.attrs,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawObject {
    name: String,
    typ: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    desc: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    attrs: Attrs,
    #[serde(flatten)]
    extra: BTreeMap<String, AttrValue>,
}

impl RawObject {
    fn with_id(self, id: i32) -> Object {
        Object {
            id,
            typ: self.typ,
            name: self.name,
            desc: self.desc,
            attrs: self.attrs,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawEvent {
    #[serde(flatten)]
    pub object: RawObject,
    pub start: OptRepeated,
    pub duration: Duration,
}

impl RawEvent {
    fn with_id(self, id: i32) -> Event {
        Event {
            object: self.object.with_id(id),
            start: self.start,
            duration: self.duration,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawTask {
    #[serde(flatten)]
    pub object: RawObject,
    pub deadline: OptRepeated,
    pub priority: u32,
}

impl RawTask {
    fn with_id(self, id: i32) -> Task {
        Task {
            object: self.object.with_id(id),
            deadline: self.deadline,
            priority: self.priority,
        }
    }
}

pub struct Storage {
    meta: Tree,
    logs: Tree,
    objs: Tree,
    handlers: SignalHandlers,
}

fn ser<S: ?Sized + serde::Serialize>(obj: &S) -> Vec<u8> {
    serde_json::to_vec(obj).unwrap()
    // bincode::serialize(obj).unwrap()
}

fn deser<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> T {
    serde_json::from_slice(bytes).unwrap()
    // bincode::deserialize(bytes).unwrap()
}

impl Storage {
    pub fn new() -> Storage {
        let config_dir = dirs::config_dir().unwrap().join("sched"); // FIXME
        let db = sled::open(config_dir.join("sched.db")).unwrap();
        let meta = db.open_tree("meta").unwrap();
        if !meta.contains_key("logs_id").unwrap() {
            meta.insert("logs_id", ser(&1i32)).unwrap();
        }
        if !meta.contains_key("objs_id").unwrap() {
            meta.insert("objs_id", ser(&1i32)).unwrap();
        }
        Storage {
            meta,
            logs: db.open_tree("logs").unwrap(),
            objs: db.open_tree("objs").unwrap(),
            handlers: SignalHandlers::new(),
        }
    }

    pub fn add_gluon(&mut self, pat: &str, f: SignalHandler) -> Result<()> {
        self.handlers.add_gluon(pat, f)
    }

    pub fn get_log_id(&mut self) -> i32 {
        deser(
            &self
                .meta
                .fetch_and_update("logs_id", |old| Some(ser(&(deser::<i32>(old.unwrap()) + 1))))
                .unwrap()
                .unwrap(),
        )
    }

    pub fn create_log(&mut self, typ: String, attrs: Attrs) -> Result<Log> {
        let id = self.get_log_id();
        let time = Utc::now().into();
        let raw = RawLog { typ, attrs, time };
        self.logs.insert(ser(&id), ser(&raw)).unwrap();
        let log = raw.with_id(id);
        self.handlers.handle(&log);
        Ok(log)
    }

    pub fn log_set_attr(&mut self, id: i32, key: String, val: AttrValue) -> Result<()> {
        self.logs
            .fetch_and_update(ser(&id), |old| {
                let mut log: RawLog = deser(old.unwrap());
                // FIXME key val cloned cuz captured by closure; use batch?
                log.attrs.entry(key.clone()).or_insert(val.clone());
                Some(ser(&log))
            })
            .unwrap();
        self.create_log("log.set_attr".into(), attrs! { "id": id, "attr": key })?;
        Ok(())
    }

    pub fn get_log(&mut self, id: i32) -> Result<Log> {
        self.logs
            .get(ser(&id))
            .unwrap()
            .map(|l| deser::<RawLog>(&l).with_id(id))
            .ok_or(Error::InvalidLogID(id))
    }

    pub fn find_log<F: Fn(&Log) -> bool>(&mut self, filter: F, limit: Option<usize>) -> Vec<Log> {
        self.logs
            .iter()
            .rev()
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<RawLog>(&v).with_id(deser(&k)))
            .filter(filter)
            .take(limit.unwrap_or(1))
            .collect()
    }

    pub fn get_obj_id(&mut self) -> i32 {
        deser(
            &self
                .meta
                .fetch_and_update("objs_id", |old| Some(ser(&(deser::<i32>(old.unwrap()) + 1))))
                .unwrap()
                .unwrap(),
        )
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert(ser(&id), ser(&json!({ "name": name, "typ": typ })))
            .unwrap();
        self.create_log("obj.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: i32, desc: String) -> Result<()> {
        let mut attrs = None;
        self.objs
            .fetch_and_update(ser(&id), |old| {
                let mut obj: RawObject = deser(old.unwrap());
                if obj.desc.is_empty() {
                    attrs = Some(attrs! { "id": id, "new": desc });
                } else {
                    attrs = Some(attrs! { "id": id, "old": obj.desc, "new": desc });
                }
                // FIXME desc cloned cuz captured by closure; use batch?
                obj.desc = desc.clone();
                Some(ser(&obj))
            })
            .unwrap();
        self.create_log("obj.set_desc".into(), attrs.unwrap())?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: i32, key: String, val: AttrValue) -> Result<()> {
        let mut attrs = None;
        self.objs
            .fetch_and_update(ser(&id), |old| {
                let mut obj: RawObject = deser(old.unwrap());
                if obj.attrs.contains_key(&key) {
                    attrs = Some(attrs! { "id": id, "old": obj.attrs[&key], "new": val });
                } else {
                    attrs = Some(attrs! { "id": id, "new": val });
                }
                // FIXME key val cloned cuz captured by closure; use batch?
                obj.attrs.insert(key.clone(), val.clone());
                Some(ser(&obj))
            })
            .unwrap();
        self.create_log("obj.set_attr".into(), attrs.unwrap())?;
        Ok(())
    }

    pub fn obj_del_attr(&mut self, id: i32, key: &str) -> Result<()> {
        let mut attrs = None;
        self.objs
            // FIXME Conditionally don't need update
            .fetch_and_update(ser(&id), |old| {
                let mut obj: RawObject = deser(old.unwrap());
                if obj.attrs.contains_key(key) {
                    attrs = Some(attrs! { "id": id, "old": obj.attrs[key] });
                    obj.attrs.remove(key);
                }
                Some(ser(&obj))
            })
            .unwrap();
        if let Some(attrs) = attrs {
            self.create_log("obj.set_attr".into(), attrs)?;
        }
        Ok(())
    }

    pub fn get_obj(&mut self, id: i32) -> Result<Object> {
        self.objs
            .get(ser(&id))
            .unwrap()
            .map(|o| deser::<RawObject>(&o).with_id(id))
            .ok_or(Error::InvalidObjID(id))
    }

    pub fn find_obj<F: Fn(&Object) -> bool>(&mut self, filter: F, limit: Option<usize>) -> Vec<Object> {
        self.objs
            .iter()
            .rev()
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<RawObject>(&v).with_id(deser(&k)))
            .filter(filter)
            .take(limit.unwrap_or(1))
            .collect()
    }

    pub fn create_task(&mut self, name: &str, typ: &str, deadline: OptRepeated, priority: u32) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert(
                ser(&id),
                ser(&json!({ "name": name, "typ": typ, "deadline": deadline, "priority": priority })),
            )
            .unwrap();
        self.create_log("task.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    pub fn get_task(&mut self, id: i32) -> Result<Task> {
        self.objs
            .get(ser(&id))
            .unwrap()
            .map(|t| deser::<RawTask>(&t).with_id(id))
            .ok_or(Error::ObjNotTask(id))
    }

    pub fn create_event(&mut self, name: &str, typ: &str, start: OptRepeated, duration: Duration) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert(
                ser(&id),
                ser(&json!({ "name": name, "typ": typ, "start": start, "duration": duration })),
            )
            .unwrap();
        self.create_log("event.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    pub fn get_event(&mut self, id: i32) -> Result<Event> {
        self.objs
            .get(ser(&id))
            .unwrap()
            .map(|e| deser::<RawEvent>(&e).with_id(id))
            .ok_or(Error::ObjNotEvent(id))
    }
}
