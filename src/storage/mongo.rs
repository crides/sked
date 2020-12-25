use bson::{doc, document::ValueAccessError, from_bson, to_bson, Bson, Document};
use chrono::Utc;
use mongodb::{
    options::FindOptions,
    sync::{Client, Collection},
};

use crate::{
    script::{
        sched::{Attr, AttrValue, Log, Object},
        task::{Event, Every, Stop, Task},
        time::{DateTime, Duration, Time},
    },
    signal::{SignalHandler, SignalHandlers},
    storage::{Error, Result},
};

pub struct Storage {
    ids: Collection,
    logs: Collection,
    objs: Collection,
    handlers: SignalHandlers,
}

impl Storage {
    pub fn new() -> Storage {
        let client = Client::with_uri_str("mongodb://localhost:27017/").expect("Can't connect to server");
        let db = client.database("sched");
        let ids = db.collection("ids");
        if ids.find_one(doc! { "_id": "logs_id" }, None).unwrap().is_none() {
            ids.insert_one(doc! { "_id": "logs_id", "id": 1i32 }, None).unwrap();
        }
        if ids.find_one(doc! { "_id": "objs_id" }, None).unwrap().is_none() {
            ids.insert_one(doc! { "_id": "objs_id", "id": 1i32 }, None).unwrap();
        }
        Storage {
            ids,
            logs: db.collection("logs"),
            objs: db.collection("objs"),
            handlers: SignalHandlers::new(),
        }
    }

    pub fn add_gluon(&mut self, pat: &str, f: SignalHandler) -> Result<()> {
        self.handlers.add_gluon(pat, f)
    }

    pub fn get_log_id(&mut self) -> i32 {
        self.ids
            .find_one_and_update(doc! { "_id": "logs_id" }, doc! { "$inc": { "id": 1 } }, None)
            .unwrap()
            .unwrap()
            .get_i32("id")
            .unwrap()
    }

    pub fn create_log(&mut self, typ: &str, attrs: Document) -> Result<Log> {
        let id = self.get_log_id();
        let time = Utc::now();
        if attrs.len() > 0 {
            self.logs
                .insert_one(
                    doc! { "_id": id, "typ": typ, "time": time, "attrs": attrs.clone() },
                    None,
                )
                .unwrap();
        } else {
            self.logs
                .insert_one(doc! { "_id": id, "typ": typ, "time": time }, None)
                .unwrap();
        }

        let log = Log {
            id,
            typ: typ.to_string(),
            attrs: from_bson(Bson::Document(attrs)).unwrap(),
            time: time.into(),
        };
        self.handlers.handle(&log);
        Ok(log)
    }

    pub fn create_log_attrs(&mut self, typ: &str, attrs: Attr) -> Result<Log> {
        let id = self.get_log_id();
        let time = Utc::now();
        if attrs.0.len() > 0 {
            self.logs
                .insert_one(
                    doc! { "_id": id, "typ": typ, "time": time, "attrs": to_bson(&attrs).unwrap() },
                    None,
                )
                .unwrap();
        } else {
            self.logs
                .insert_one(doc! { "_id": id, "typ": typ, "time": time }, None)
                .unwrap();
        }

        let log = Log {
            id,
            typ: typ.to_string(),
            attrs,
            time: time.into(),
        };
        self.handlers.handle(&log);
        Ok(log)
    }

    pub fn log_set_attr(&mut self, id: i32, key: &str, val: AttrValue) -> Result<()> {
        if key.contains('.') {
            return Err(Error::InvalidKey(key.to_string()));
        }
        let key = format!("attrs.{}", key);
        self.logs
            .find_one_and_update(
                doc! { "_id": id, key.clone(): { "$exists": false } },
                doc! { "$set": { key.clone(): to_bson(&val).unwrap() } },
                None,
            )
            .unwrap();
        self.create_log("log.set_attr", doc! { "id": id, "attr": key })?;
        Ok(())
    }

    pub fn get_log(&mut self, id: i32) -> Result<Log> {
        let log = self
            .logs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidLogID(id))?;
        Ok(from_bson(Bson::Document(log)).unwrap())
    }

    pub fn find_log(&mut self, filter: Document, limit: Option<usize>) -> Vec<Log> {
        self.logs
            .find(
                Some(filter),
                Some(FindOptions::builder().sort(doc! { "_id": -1 }).build()),
            )
            .unwrap()
            .take(limit.unwrap_or(std::usize::MAX))
            .map(|l| from_bson(Bson::Document(l.unwrap())).unwrap())
            .collect()
    }

    pub fn get_obj_id(&mut self) -> i32 {
        self.ids
            .find_one_and_update(doc! { "_id": "objs_id" }, doc! { "$inc": { "id": 1 } }, None)
            .unwrap()
            .unwrap()
            .get_i32("id")
            .unwrap()
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "typ": typ }, None)
            .unwrap();
        self.create_log("obj.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: i32, desc: &str) -> Result<()> {
        let old_obj = self
            .objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$set": { "desc": desc } }, None)
            .unwrap()
            .unwrap();
        let attrs = match old_obj.get_str("desc") {
            Ok(old) => {
                doc! { "id": id, "old": old, "new": desc }
            }
            Err(ValueAccessError::NotPresent) => {
                doc! { "id": id, "new": desc }
            }
            _ => unreachable!(),
        };
        self.create_log("obj.set_desc", attrs)?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: i32, key: &str, val: AttrValue) -> Result<()> {
        if key.contains('.') {
            return Err(Error::InvalidKey(key.to_string()));
        }
        let val = to_bson(&val).unwrap();
        let old_obj = self
            .objs
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { format!("attrs.{}", key): val.clone() } },
                None,
            )
            .unwrap()
            .unwrap();
        let attrs = match old_obj.get_document("attrs").map(|d| d.get_str(key)) {
            Ok(Ok(old)) => {
                doc! { "key": key, "id": id, "old": old, "new": val }
            }
            Err(ValueAccessError::NotPresent) | Ok(Err(ValueAccessError::NotPresent)) => {
                doc! { "key": key, "id": id, "new": val }
            }
            _ => unreachable!(),
        };
        self.create_log("obj.set_attr", attrs)?;
        Ok(())
    }

    pub fn obj_del_attr(&mut self, id: i32, key: &str) -> Result<()> {
        if key.contains('.') {
            return Err(Error::InvalidKey(key.to_string()));
        }
        let old_obj = self
            .objs
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$unset": { format!("attrs.{}", key): 0 } },
                None,
            )
            .unwrap()
            .unwrap();
        match old_obj.get_document("attrs").map(|d| d.get_str(key).unwrap()) {
            Ok(old) => {
                self.create_log("obj.del_attr", doc! { "id": id, "key": key, "old": old })?;
            }
            _ => (),
        }
        Ok(())
    }

    pub fn get_obj(&mut self, id: i32) -> Result<Object> {
        let obj = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        Ok(from_bson(Bson::Document(obj)).unwrap())
    }

    pub fn find_obj(&mut self, filter: Document, limit: Option<usize>) -> Vec<Object> {
        self.objs
            .find(
                Some(filter),
                Some(FindOptions::builder().sort(doc! { "_id": -1 }).build()),
            )
            .unwrap()
            .take(limit.unwrap_or(std::usize::MAX))
            .map(|o| from_bson(Bson::Document(o.unwrap())).unwrap())
            .collect()
    }

    pub fn create_task(
        &mut self,
        name: &str,
        start: DateTime,
        every: Every,
        stop: Stop,
        deadline: Time,
        priority: u32,
    ) -> Result<i32> {
        let id = self.get_obj_id();
        let start = DateTime(start.0.date().and_hms(0, 0, 0));
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "typ": "task", "start": start.to_utc(), "every": every.to_doc(), "stop": stop.to_doc(), "deadline": deadline.to_secs(), "priority": priority }, None)
            .unwrap();
        self.create_log("task.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn get_task(&mut self, id: i32) -> Result<Task> {
        let task = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        if task.get_str("typ") != Ok("task") {
            return Err(Error::ObjNotTask(id));
        }
        let start = task.get_datetime("start").unwrap().clone().into();
        let every = Every::from_doc(task.get_document("every").unwrap().clone());
        let stop = Stop::from_doc(task.get_document("stop").unwrap().clone());
        let deadline = Time::from_secs(task.get_i32("deadline").unwrap() as u32).unwrap();
        let priority = task.get_i32("priority").unwrap() as u32;
        let object = from_bson(Bson::Document(task)).unwrap();
        Ok(Task {
            start,
            every,
            stop,
            deadline,
            object,
            priority,
        })
    }

    pub fn create_event(
        &mut self,
        name: &str,
        start: DateTime,
        every: Every,
        stop: Stop,
        event_start: Time,
        duration: Duration,
    ) -> Result<i32> {
        let id = self.get_obj_id();
        let start = DateTime(start.0.date().and_hms(0, 0, 0));
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "typ": "event", "start": start.to_utc(), "every": every.to_doc(), "stop": stop.to_doc(), "event_start": event_start.to_secs(), "duration": duration.num_seconds() }, None)
            .unwrap();
        self.create_log("event.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn get_event(&mut self, id: i32) -> Result<Event> {
        let task = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        if task.get_str("typ") != Ok("event") {
            return Err(Error::ObjNotEvent(id));
        }
        let start = task.get_datetime("start").unwrap().clone().into();
        let every = Every::from_doc(task.get_document("every").unwrap().clone());
        let stop = Stop::from_doc(task.get_document("stop").unwrap().clone());
        let event_start = Time::from_secs(task.get_i32("event_start").unwrap() as u32).unwrap();
        let duration = Duration::seconds(task.get_i64("duration").unwrap());
        let object = from_bson(Bson::Document(task)).unwrap();
        Ok(Event {
            start,
            every,
            stop,
            event_start,
            duration,
            object,
        })
    }
}
