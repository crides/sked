use std::collections::BTreeMap;
use std::convert::TryInto;

use chrono::{TimeZone, Utc};
use serde_json::json;
use sled::Tree;

use crate::{
    script::{
        sched::{AttrValue, Attrs, Log, Object},
        task::{Event, Task},
        time::{DateTime, Duration},
    },
    signal::{SignalHandler, SignalHandlers},
    storage::{Error, OptRepeated, Result},
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
    /// UTC time for when the log happened
    time: DateTime,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    attrs: Attrs,
}

impl RawLog {
    fn with_id(self, id: u32) -> Log {
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
}

impl RawObject {
    fn with_id(self, id: u32) -> Object {
        Object {
            id,
            typ: self.typ,
            name: self.name,
            desc: self.desc,
            attrs: self.attrs,
        }
    }
}

impl From<Object> for RawObject {
    fn from(o: Object) -> RawObject {
        RawObject {
            typ: o.typ,
            name: o.name,
            desc: o.desc,
            attrs: o.attrs,
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
    fn with_id(self, id: u32) -> Event {
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
    #[serde(rename = "task-typ")]
    pub task_typ: String,
    /// A fixed-size FIFO cache of the daughter task ids with user configurable size
    pub cache: Vec<u32>,
}

impl RawTask {
    fn with_id(self, id: u32) -> Task {
        Task {
            object: self.object.with_id(id),
            deadline: self.deadline,
            priority: self.priority,
            task_typ: self.task_typ,
            cache: self.cache,
        }
    }
}

impl From<Task> for RawTask {
    fn from(t: Task) -> RawTask {
        RawTask {
            object: t.object.into(),
            deadline: t.deadline,
            priority: t.priority,
            task_typ: t.task_typ,
            cache: t.cache,
        }
    }
}

// FIXME limit range of logs to only logs or handlers
pub struct Storage {
    meta: Tree,
    logs: Tree,
    objs: Tree,
    handlers: SignalHandlers,
}

fn ser<S: ?Sized + serde::Serialize>(obj: &S) -> Vec<u8> {
    serde_json::to_vec(obj).unwrap()
}

fn deser<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> T {
    serde_json::from_slice(bytes).unwrap()
}

fn ser_id(id: u32) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

fn deser_id(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("malformed id in binary"))
}

impl Storage {
    pub fn new() -> Storage {
        let config_dir = dirs::config_dir().unwrap().join("sched"); // FIXME
        let db = sled::open(config_dir.join("sched.db")).unwrap();
        let meta = db.open_tree("meta").unwrap();
        if !meta.contains_key("logs_id").unwrap() {
            meta.insert("logs_id", ser_id(1u32)).unwrap();
        }
        if !meta.contains_key("objs_id").unwrap() {
            meta.insert("objs_id", ser_id(1u32)).unwrap();
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

    fn get_log_id(&mut self) -> u32 {
        deser_id(
            &self
                .meta
                .fetch_and_update("logs_id", |old| Some(ser_id(deser_id(old.unwrap()) + 1)))
                .unwrap()
                .unwrap(),
        )
    }

    pub fn create_log(&mut self, typ: String, attrs: Attrs) -> Result<u32> {
        let id = self.get_log_id();
        let time = Utc::now().into();
        let raw = RawLog { typ, attrs, time };
        self.logs.insert(ser_id(id), ser(&raw)).unwrap();
        let log = raw.with_id(id);
        self.handlers.handle(&log);
        Ok(id)
    }

    pub fn log_add_attr_raw(&mut self, id: u32, key: String, val: AttrValue) -> Result<()> {
        self.logs
            .fetch_and_update(ser_id(id), |old| {
                let mut log: RawLog = deser(old.unwrap());
                // FIXME key val cloned cuz captured by closure; use batch?
                log.attrs.entry(key.clone()).or_insert(val.clone());
                Some(ser(&log))
            })
            .unwrap();
        Ok(())
    }

    /// Add an attribute to a log. This is useful because sometimes not all information is available at the
    /// time of log creation
    pub fn log_add_attr(&mut self, id: u32, key: String, val: AttrValue) -> Result<()> {
        self.log_add_attr_raw(id, key.clone(), val)?;
        self.create_log("log.set_attr".into(), attrs! { "id": id, "attr": key })?;
        Ok(())
    }

    pub fn get_log(&mut self, id: u32) -> Result<Log> {
        self.logs
            .get(ser_id(id))
            .unwrap()
            .map(|l| deser::<RawLog>(&l).with_id(id))
            .ok_or(Error::InvalidLogID(id))
    }

    pub fn find_log<F: Fn(&Log) -> bool>(&mut self, filter: F, limit: Option<usize>) -> Vec<Log> {
        self.logs
            .iter()
            .rev()
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<RawLog>(&v).with_id(deser_id(&k)))
            .filter(filter)
            .take(limit.unwrap_or(1))
            .collect()
    }

    fn get_obj_id(&mut self) -> u32 {
        deser_id(
            &self
                .meta
                .fetch_and_update("objs_id", |old| Some(ser_id(deser_id(old.unwrap()) + 1)))
                .unwrap()
                .unwrap(),
        )
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> Result<u32> {
        let id = self.get_obj_id();
        self.objs
            .insert(ser_id(id), ser(&json!({ "name": name, "typ": typ })))
            .unwrap();
        self.create_log("obj.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: u32, desc: String) -> Result<()> {
        let mut attrs = None;
        self.objs
            .fetch_and_update(ser_id(id), |old| {
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

    pub fn obj_set_attr(&mut self, id: u32, key: String, val: AttrValue) -> Result<()> {
        let mut attrs = None;
        self.objs
            .fetch_and_update(ser_id(id), |old| {
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

    pub fn obj_del_attr(&mut self, id: u32, key: &str) -> Result<()> {
        let mut attrs = None;
        self.objs
            // FIXME Conditionally don't need update
            .fetch_and_update(ser_id(id), |old| {
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

    pub fn get_obj(&mut self, id: u32) -> Result<Object> {
        self.objs
            .get(ser_id(id))
            .unwrap()
            .map(|o| deser::<RawObject>(&o).with_id(id))
            .ok_or(Error::InvalidObjID(id))
    }

    pub fn find_obj<F: Fn(&Object) -> bool>(&mut self, filter: F, limit: Option<usize>) -> Vec<Object> {
        self.objs
            .iter()
            .rev()
            .map(|res| res.unwrap())
            .map(|(k, v)| deser::<RawObject>(&v).with_id(deser_id(&k)))
            .filter(filter)
            .take(limit.unwrap_or(1))
            .collect()
    }

    pub fn create_task(
        &mut self,
        name: &str,
        typ: &str,
        deadline: OptRepeated,
        priority: u32,
        attrs: Option<Attrs>,
    ) -> Result<u32> {
        // FIXME use batch (atomic) or transaction sematics
        let id = self.get_obj_id();
        let mut task = RawTask {
            object: RawObject {
                name: name.into(),
                typ: "task".into(),
                desc: "".into(),
                attrs: attrs.unwrap_or_default(),
            },
            deadline,
            task_typ: typ.into(),
            priority,
            cache: Vec::new(),
        };
        match task.deadline {
            OptRepeated::Single(time) => {
                let new_id = self.new_daughter_task(id, time)?;
                task.cache.push(new_id);
            }
            OptRepeated::Repeat(ref mut repeat) => {
                // FIXME attribute casting should be an system error and should create log entry
                let gen_ahead = task
                    .object
                    .attrs
                    .get("gen-ahead")
                    .map(|v| v.as_u64())
                    .flatten()
                    .unwrap_or(5);
                for _ in 0..gen_ahead {
                    if let Some(next_time) = repeat.next() {
                        let new_id = self.new_daughter_task(id, next_time)?;
                        task.cache.push(new_id);
                    } else {
                        break;
                    }
                }
            }
        }
        self.objs.insert(ser_id(id), ser(&task)).unwrap();
        self.create_log("task.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    fn new_daughter_task(&mut self, id: u32, deadline: DateTime) -> Result<u32> {
        self.create_log("task.task".into(), attrs! { "task-id": id, "deadline": deadline })
    }

    fn get_raw_task(&mut self, id: u32) -> Result<RawTask> {
        self.objs
            .get(ser_id(id))
            .unwrap()
            .map(|t| deser::<RawTask>(&t))
            .ok_or(Error::ObjNotTask(id))
    }

    pub fn get_task(&mut self, id: u32) -> Result<Task> {
        self.get_raw_task(id).map(|t| t.with_id(id))
    }

    // FIXME Error on finished tasks? Or how to handle collision
    pub fn task_finish(&mut self, id: u32, finished: DateTime) -> Result<()> {
        self.log_add_attr_raw(id, "finished".into(), serde_json::to_value(finished).unwrap())?;
        let task_log_id = self
            .get_log(id)?
            .attrs
            .get("task-id")
            .map(|v| v.as_u64())
            .flatten()
            .expect("task id from task log") as u32;
        let mut task = self.get_raw_task(task_log_id)?;
        if let OptRepeated::Repeat(ref mut repeat) = task.deadline {
            let gen_ahead = task
                .object
                .attrs
                .get("gen-ahead")
                .map(|v| v.as_u64())
                .flatten()
                .unwrap_or(5);
            let cache_size = task
                .object
                .attrs
                .get("cache-size")
                .map(|v| v.as_u64())
                .flatten()
                .unwrap_or(10);
            let cache_size = cache_size + gen_ahead + 1;
            if let Some(next_time) = repeat.next() {
                let new_id = self.new_daughter_task(id, next_time)?;
                // We only generate one cuz there can be only 1 task completed
                task.cache.push(new_id);
                if task.cache.len() > cache_size as usize {
                    task.cache.remove(0);
                }
            }
            self.objs.insert(ser_id(id), ser(&task)).unwrap();
        }
        self.create_log("task.finish".into(), attrs! { "id": id })?;
        Ok(())
    }

    pub fn create_event(
        &mut self,
        name: &str,
        typ: &str,
        start: OptRepeated,
        duration: Duration,
        attrs: Option<Attrs>,
    ) -> Result<u32> {
        let id = self.get_obj_id();
        let j = if let Some(attrs) = attrs {
            json!({ "name": name, "typ": "event", "task-typ": typ, "start": start, "duration": duration, "attrs": attrs })
        } else {
            json!({ "name": name, "typ": "event", "task-typ": typ, "start": start, "duration": duration })
        };
        self.objs.insert(ser_id(id), ser(&j)).unwrap();
        self.create_log("event.create".into(), attrs! { "id": id })?;
        Ok(id)
    }

    pub fn get_event(&mut self, id: u32) -> Result<Event> {
        self.objs
            .get(ser_id(id))
            .unwrap()
            .map(|e| deser::<RawEvent>(&e).with_id(id))
            .ok_or(Error::ObjNotEvent(id))
    }

    pub fn find_current(&mut self, id: u32) -> Result<Option<u32>> {
        // It should
        let current_utc = Utc::now();
        let task = self.get_raw_task(id)?;
        // FIXME better name?
        let balanced = task
            .object
            .attrs
            .get("flavor")
            .map(|v| v == "balanced")
            .unwrap_or(false);
        let unfinished = task
            .cache
            .iter()
            .map(|&i| self.get_log(i).unwrap())
            .filter(|l| !l.attrs.contains_key("finished"))
            .collect::<Vec<_>>();

        let deadlines = unfinished
            .iter()
            .map(|task| Utc.timestamp(task.attrs.get("deadline").unwrap().as_i64().unwrap(), 0))
            .collect::<Vec<_>>();
        let len = unfinished.len();
        let grace = chrono::Duration::minutes(5);
        // TODO sort this instead so that past unfinished tasks maybe current?
        for i in 0..len {
            let deadline = deadlines[i];
            let criterion = if balanced {
                i == len - 1 || (deadline + (deadlines[i + 1] - deadline) / 2) > current_utc
            } else {
                current_utc < deadline + grace
            };
            if criterion {
                return Ok(Some(unfinished[i].id));
            }
        }
        Ok(None)
    }
}
