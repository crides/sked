use bson::{doc, from_bson};
use chrono::Utc;
use mongodb::{
    error::Result,
    sync::{Client, Collection},
};

use crate::storage::*;

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
        self.logs
            .insert_one(doc! { "_id": id, "name": name, "type": typ, "time": Utc::now() }, None)?;
        Ok(id)
    }

    pub fn log_set_desc(&mut self, id: i32, desc: &str) -> Result<()> {
        self.logs.find_one_and_update(doc! { "_id": id }, doc! { "$set": { "desc": desc } }, None)?;
        Ok(())
    }

    pub fn log_set_obj(&mut self, id: i32, obj: ObjectRef) -> Result<()> {
        self.logs.find_one_and_update(doc! { "_id": id }, doc! { "$set": { "obj": obj } }, None)?;
        Ok(())
    }

    pub fn get_log(&mut self, id: i32) -> Result<Log> {
        let log = self.logs.find_one(doc! { "_id": id }, None)?.expect("no log with such id");
        Ok(from_bson(dbg!(dbg!(log).into()))?)
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
            .insert_one(doc! { "_id": id, "name": name, "typ": typ }, None)?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: ObjectRef, desc: &str) -> Result<()> {
        self.objs.find_one_and_update(doc! { "_id": id }, doc! { "$set": { "desc": desc } }, None)?;
        Ok(())
    }

    pub fn obj_add_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(doc! { "_id": id }, doc! { "$push": { "deps": dep } }, None)?;
        Ok(())
    }

    pub fn obj_add_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(doc! { "_id": id }, doc! { "$push": { "subs": sub } }, None)?;
        Ok(())
    }

    pub fn obj_add_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> Result<()> {
        self.objs.find_one_and_update(doc! { "_id": id }, doc! { "$push": { "refs": rf } }, None)?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: ObjectRef, name: &str, val: &str) -> Result<()> {
        self.objs.find_one_and_update(doc! { "_id": id }, doc! { "$set": { "attrs": { name: val } } }, None)?;
        Ok(())
    }

    pub fn get_obj(&mut self, id: ObjectRef) -> Result<Object> {
        Ok(from_bson(self.logs.find_one(doc! { "_id": id }, None)?.expect("no log with such id").into())?)
    }
}
