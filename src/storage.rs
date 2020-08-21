use std::collections::HashMap;

use anyhow::{anyhow, Result};
use bson::{doc, document::ValueAccessError, from_bson, Document};
use chrono::{DateTime, Utc};
use mongodb::sync::{Client, Collection};
use rlua::prelude::*;

use crate::event::EventHandlers;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    pub time: DateTime<Utc>,
    #[serde(default)]
    pub attrs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub name: String,
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    pub desc: Option<String>,
    #[serde(default)]
    pub deps: Vec<ObjectRef>,
    #[serde(default)]
    pub subs: Vec<ObjectRef>,
    #[serde(default)]
    pub refs: Vec<ObjectRef>,
    #[serde(default)]
    pub attrs: HashMap<String, String>,
}

pub type ObjectRef = i32;

pub struct Storage<'lua> {
    ids: Collection,
    logs: Collection,
    objs: Collection,
    handlers: EventHandlers<'lua>
}

impl<'lua> Storage<'lua> {
    pub fn new() -> Result<Storage<'lua>> {
        let client = Client::with_uri_str("mongodb://localhost:27017/")?;
        let db = client.database("sched");
        let ids = db.collection("ids");
        if ids.find_one(doc! { "_id": "logs_id" }, None)?.is_none() {
            ids.insert_one(doc! { "_id": "logs_id", "id": 1i32 }, None)?;
        }
        if ids.find_one(doc! { "_id": "objs_id" }, None)?.is_none() {
            ids.insert_one(doc! { "_id": "objs_id", "id": 1i32 }, None)?;
        }
        Ok(Storage {
            ids,
            logs: db.collection("logs"),
            objs: db.collection("objs"),
            handlers: EventHandlers::new(),
        })
    }

    pub fn add_lua(&mut self, pat: &str, f: LuaFunction<'lua>) -> Result<()> {
        self.handlers.add_lua(pat, f)
    }

    pub fn create_log(&mut self, typ: &str, attrs: Document) -> Result<i32> {
        let id = self
            .ids
            .find_one_and_update(
                doc! { "_id": "logs_id" },
                doc! { "$inc": { "id": 1 } },
                None,
            )?
            .unwrap()
            .get_i32("id")
            .unwrap();
        // let attrs = attrs.into_iter().map(|(k, v)| (k, Bson::String(v))).collect::<Document>();
        if attrs.len() > 0 {
            self.logs.insert_one(
                doc! { "_id": id, "type": typ, "time": Utc::now(), "attrs": attrs },
                None,
            )?;
        } else {
            self.logs
                .insert_one(doc! { "_id": id, "type": typ, "time": Utc::now() }, None)?;
        }

        // FIXME optimize this
        self.handlers.handle(&self.get_log(id)?);
        Ok(id)
    }

    pub fn log_set_attr(&mut self, id: i32, key: &str, val: &str) -> Result<()> {
        if key.contains('.') {
            return Err(anyhow!("Invalid attr key: {}", key));
        }
        let key = format!("attrs.{}", key);
        self.logs.find_one_and_update(
            doc! { "_id": id, key.clone(): { "$exists": false } },
            doc! { "$set": { key.clone(): val } },
            None,
        )?;
        self.create_log("log.set_attr", doc! { "id": id, "attr": key })?;
        Ok(())
    }

    pub fn get_log(&mut self, id: i32) -> Result<Log> {
        let log = self
            .logs
            .find_one(doc! { "_id": id }, None)?
            .ok_or_else(|| anyhow!("No such log id: {}", id))?;
        // FIXME The deser impl in `Bson` is missing for `Datetime<>`.
        // Github issue: https://github.com/mongodb/bson-rust/issues/191, and
        // tracking Jira in MongoDB: https://jira.mongodb.org/browse/RUST-506
        Ok(Log {
            typ: log.get_str("type").unwrap().into(),
            time: log.get_datetime("time").unwrap().clone(),
            attrs: log
                .get_document("attrs")
                .map(|d| {
                    d.into_iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> Result<ObjectRef> {
        let id = self
            .ids
            .find_one_and_update(
                doc! { "_id": "objs_id" },
                doc! { "$inc": { "id": 1 } },
                None,
            )?
            .unwrap()
            .get_i32("id")
            .unwrap();
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "type": typ }, None)?;
        self.create_log("obj.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: ObjectRef, desc: &str) -> Result<()> {
        let old_obj = self
            .objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$set": { "desc": desc } }, None)?
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

    pub fn obj_add_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "deps": dep } },
            None,
        )?;
        self.create_log("obj.add_dep", doc! { "id": id, "dep": dep })?;
        Ok(())
    }

    pub fn obj_add_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "subs": sub } },
            None,
        )?;
        self.create_log("obj.add_sub", doc! { "sub": sub, "id": id })?;
        Ok(())
    }

    pub fn obj_add_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "refs": rf } },
            None,
        )?;
        self.create_log("obj.add_ref", doc! { "ref": rf, "id": id })?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: ObjectRef, key: &str, val: &str) -> Result<()> {
        if key.contains('.') {
            return Err(anyhow!("Invalid attr key: {}", key));
        }
        let old_obj = self
            .objs
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { format!("attrs.{}", key): val } },
                None,
            )?
            .unwrap();
        let attrs = match old_obj
            .get_document("attrs")
            .map(|d| d.get_str(key).unwrap())
        {
            Ok(old) => {
                doc! { "key": key, "id": id, "old": old, "new": val }
            }
            Err(ValueAccessError::NotPresent) => {
                doc! { "key": key, "id": id, "new": val }
            }
            _ => unreachable!(),
        };
        self.create_log("obj.set_attr", attrs)?;
        Ok(())
    }

    pub fn obj_del_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "deps": dep } },
            None,
        )?;
        self.create_log("obj.del_dep", doc! { "dep": dep, "id": id })?;
        Ok(())
    }

    pub fn obj_del_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "subs": sub } },
            None,
        )?;
        self.create_log("obj.del_sub", doc! { "sub": sub, "id": id })?;
        Ok(())
    }

    pub fn obj_del_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "refs": rf } },
            None,
        )?;
        self.create_log("obj.del_ref", doc! { "ref": rf, "id": id })?;
        Ok(())
    }

    pub fn obj_del_attr(&mut self, id: ObjectRef, key: &str) -> Result<()> {
        if key.contains('.') {
            return Err(anyhow!("Invalid attr key: {}", key));
        }
        let old_obj = self
            .objs
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$unset": { format!("attrs.{}", key): 0 } },
                None,
            )?
            .unwrap();
        match old_obj
            .get_document("attrs")
            .map(|d| d.get_str(key).unwrap())
        {
            Ok(old) => {
                self.create_log("obj.del_attr", doc! { "id": id, "key": key, "old": old })?;
            }
            _ => (),
        }
        Ok(())
    }

    pub fn get_obj(&mut self, id: ObjectRef) -> Result<Object> {
        let obj = self
            .objs
            .find_one(doc! { "_id": id }, None)?
            .ok_or_else(|| anyhow!("No such obj id: {}", id))?;
        Ok(from_bson(obj.into())?)
    }
}
