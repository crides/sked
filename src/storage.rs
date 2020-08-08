use std::collections::HashMap;

use anyhow::{anyhow, Result};
use bson::{doc, from_bson, document::ValueAccessError};
use chrono::{DateTime, Utc};
use mongodb::sync::{Client, Collection};


#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub name: String,
    #[serde(rename(deserialize = "type"))]
    pub typ: String,
    pub time: DateTime<Utc>,
    pub desc: Option<String>,
    pub obj: Option<ObjectRef>,
}

#[derive(Debug, Serialize, Deserialize)]
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

pub struct Storage {
    ids: Collection,
    logs: Collection,
    objs: Collection,
}

impl Storage {
    pub fn new() -> Result<Storage> {
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
        })
    }

    pub fn create_log(&mut self, name: &str, typ: &str) -> Result<i32> {
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
        self.logs.insert_one(
            doc! { "_id": id, "name": name, "type": typ, "time": Utc::now() },
            None,
        )?;
        Ok(id)
    }

    pub fn log_set_desc(&mut self, id: i32, desc: &str) -> Result<()> {
        self.logs.find_one_and_update(
            doc! { "_id": id, "desc": { "$exists": false } },
            doc! { "$set": { "desc": desc } },
            None,
        )?;
        Ok(())
    }

    pub fn log_set_obj(&mut self, id: i32, obj: ObjectRef) -> Result<()> {
        self.logs
            .find_one_and_update(doc! { "_id": id, "obj": { "$exists": false } }, doc! { "$set": { "obj": obj } }, None)?;
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
            name: log.get_str("name").unwrap().into(),
            typ: log.get_str("type").unwrap().into(),
            time: log.get_datetime("time").unwrap().clone(),
            desc: log.get_str("desc").ok().map(|s| s.into()),
            obj: log.get_i32("obj").ok(),
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
        self.create_log(&format!("Created object #{}", id), "obj.create")?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: ObjectRef, desc: &str) -> Result<()> {
        let old_obj = self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$set": { "desc": desc } },
            None,
        )?.unwrap();
        match old_obj.get_str("desc") {
            Ok(old) => {
                let id = self.create_log(&format!("Changed description for obj #{}", id), "obj.set_desc")?;
                self.log_set_desc(id, &format!("Old description:\n {}\n\nNew description:\n {}", old, desc))?;
            }
            Err(ValueAccessError::NotPresent) => {
                let id = self.create_log(&format!("Added description for obj #{}", id), "obj.set_desc")?;
                self.log_set_desc(id, &format!("New description:\n {}", desc))?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    pub fn obj_add_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "deps": dep } },
            None,
        )?;
        self.create_log(&format!("Added dependency #{} for object #{}", dep, id), "obj.add_dep")?;
        Ok(())
    }

    pub fn obj_add_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "subs": sub } },
            None,
        )?;
        self.create_log(&format!("Added subobject #{} for object #{}", sub, id), "obj.add_sub")?;
        Ok(())
    }

    pub fn obj_add_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$addToSet": { "refs": rf } },
            None,
        )?;
        self.create_log(&format!("Added reference #{} for object #{}", rf, id), "obj.add_ref")?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: ObjectRef, key: &str, val: &str) -> Result<()> {
        if key.contains('.') {
            return Err(anyhow!("Invalid attr key: {}", key));
        }
        let old_obj = self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$set": { format!("attrs.{}", key): val } },
            None,
        )?.unwrap();
        match old_obj.get_document("attrs").map(|d| d.get_str(key).unwrap()) {
            Ok(old) => {
                let id = self.create_log(&format!("Changed attribute '{}' for obj #{}", key, id), "obj.set_attr")?;
                self.log_set_desc(id, &format!("Old value:\n {}\n\nNew value:\n {}", old, val))?;
            }
            Err(ValueAccessError::NotPresent) => {
                let id = self.create_log(&format!("Set attribute '{}' for obj #{}", key, id), "obj.set_desc")?;
                self.log_set_desc(id, &format!("New value:\n {}", val))?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    pub fn obj_del_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "deps": dep } },
            None,
        )?;
        self.create_log(&format!("Deleted dependency #{} for object #{}", dep, id), "obj.del_dep")?;
        Ok(())
    }

    pub fn obj_del_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "subs": sub } },
            None,
        )?;
        self.create_log(&format!("Deleted subobject #{} for object #{}", sub, id), "obj.del_sub")?;
        Ok(())
    }

    pub fn obj_del_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$pull": { "refs": rf } },
            None,
        )?;
        self.create_log(&format!("Deleted reference #{} for object #{}", rf, id), "obj.del_ref")?;
        Ok(())
    }

    pub fn obj_del_attr(&mut self, id: ObjectRef, key: &str) -> Result<()> {
        if key.contains('.') {
            return Err(anyhow!("Invalid attr key: {}", key));
        }
        let old_obj = self.objs.find_one_and_update(
            doc! { "_id": id },
            doc! { "$unset": { format!("attrs.{}", key): 0 } },
            None,
        )?.unwrap();
        match old_obj.get_document("attrs").map(|d| d.get_str(key).unwrap()) {
            Ok(old) => {
                let id = self.create_log(&format!("Deleted attribute '{}' for obj #{}", key, id), "obj.set_attr")?;
                self.log_set_desc(id, &format!("Old value:\n {}", old))?;
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
