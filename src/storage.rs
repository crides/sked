use std::collections::BTreeMap;

use bson::{doc, document::ValueAccessError, Bson, Document};
use chrono::Utc;
use mongodb::sync::{Client, Collection};

use crate::script::time::DateTime as GluonDateTime;
use crate::signal::{SignalHandler, SignalHandlers};

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum AttrValue {
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub enum Error {
    Regex(String),
    InvalidKey(String),
    InvalidLogID(i32),
    InvalidObjID(i32),
}

pub type Result<T> = std::result::Result<T, Error>;
pub type Attr = BTreeMap<String, AttrValue>;

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub struct Log {
    pub id: i32,
    pub typ: String,
    pub time: GluonDateTime,
    pub attrs: Attr,
}

#[derive(Clone, Debug, Trace, VmType, Pushable, Getable)]
#[gluon_trace(skip)]
pub struct Object {
    pub id: i32,
    pub name: String,
    pub typ: String,
    pub desc: Option<String>,
    pub deps: Vec<i32>,
    pub subs: Vec<i32>,
    pub refs: Vec<i32>,
    pub attrs: Attr,
}

pub struct Storage {
    ids: Collection,
    logs: Collection,
    objs: Collection,
    handlers: SignalHandlers,
}

pub fn attr_to_bson(v: AttrValue) -> Bson {
    match v {
        AttrValue::Int(i) => Bson::Int64(i),
        AttrValue::Float(f) => Bson::Double(f),
        AttrValue::String(s) => Bson::String(s),
    }
}

pub fn doc_to_attr(d: Document) -> Attr {
    d.into_iter()
        .map(|(k, v)| {
            (
                k,
                match v {
                    Bson::String(s) => AttrValue::String(s),
                    Bson::Double(f) => AttrValue::Float(f),
                    Bson::Int64(i) => AttrValue::Int(i),
                    Bson::Int32(i) => AttrValue::Int(i as i64),
                    _ => panic!("expected string but got: {:?}", v),
                },
            )
        })
        .collect()
}

pub fn attr_to_doc(a: Attr) -> Document {
    a.into_iter().map(|(k, v)| (k, attr_to_bson(v))).collect()
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

    pub fn create_log(&mut self, typ: &str, attrs: Document) -> Result<Log> {
        let id = self
            .ids
            .find_one_and_update(doc! { "_id": "logs_id" }, doc! { "$inc": { "id": 1 } }, None)
            .unwrap()
            .unwrap()
            .get_i32("id")
            .unwrap();
        let time = Utc::now();
        if attrs.len() > 0 {
            self.logs
                .insert_one(
                    doc! { "_id": id, "type": typ, "time": time, "attrs": attrs.clone() },
                    None,
                )
                .unwrap();
        } else {
            self.logs
                .insert_one(doc! { "_id": id, "type": typ, "time": time }, None)
                .unwrap();
        }

        let log = Log {
            id,
            typ: typ.to_string(),
            attrs: doc_to_attr(attrs),
            time: GluonDateTime(time.into()),
        };
        self.handlers.handle(&log);
        Ok(log)
    }

    pub fn create_log_attrs(&mut self, typ: &str, attrs: Attr) -> Result<Log> {
        let id = self
            .ids
            .find_one_and_update(doc! { "_id": "logs_id" }, doc! { "$inc": { "id": 1 } }, None)
            .unwrap()
            .unwrap()
            .get_i32("id")
            .unwrap();
        let time = Utc::now();
        if attrs.len() > 0 {
            self.logs
                .insert_one(
                    doc! { "_id": id, "type": typ, "time": time, "attrs": attr_to_doc(attrs.clone()) },
                    None,
                )
                .unwrap();
        } else {
            self.logs
                .insert_one(doc! { "_id": id, "type": typ, "time": time }, None)
                .unwrap();
        }

        let log = Log {
            id,
            typ: typ.to_string(),
            attrs,
            time: GluonDateTime(time.into()),
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
                doc! { "$set": { key.clone(): attr_to_bson(val) } },
                None,
            )
            .unwrap();
        self.create_log("log.set_attr", doc! { "id": id, "attr": key })?;
        Ok(())
    }

    pub fn get_log(&mut self, id: i32) -> Result<Log> {
        let mut log = self
            .logs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidLogID(id))?;
        Ok(Log {
            id,
            typ: match log.remove("type").unwrap() {
                Bson::String(s) => s,
                _ => panic!("Expected string for type!"),
            },
            time: match log.remove("time").unwrap() {
                Bson::DateTime(t) => GluonDateTime(t.into()),
                _ => panic!("Expected DateTime for time!"),
            },
            attrs: match log.remove("attrs") {
                Some(Bson::Document(d)) => doc_to_attr(d),
                _ => Attr::default(),
            },
        })
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> Result<i32> {
        let id = self
            .ids
            .find_one_and_update(doc! { "_id": "objs_id" }, doc! { "$inc": { "id": 1 } }, None)
            .unwrap()
            .unwrap()
            .get_i32("id")
            .unwrap();
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "type": typ }, None)
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

    pub fn obj_add_dep(&mut self, id: i32, dep: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$addToSet": { "deps": dep } }, None)
            .unwrap();
        self.create_log("obj.add_dep", doc! { "id": id, "dep": dep })?;
        Ok(())
    }

    pub fn obj_add_sub(&mut self, id: i32, sub: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$addToSet": { "subs": sub } }, None)
            .unwrap();
        self.create_log("obj.add_sub", doc! { "sub": sub, "id": id })?;
        Ok(())
    }

    pub fn obj_add_ref(&mut self, id: i32, rf: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$addToSet": { "refs": rf } }, None)
            .unwrap();
        self.create_log("obj.add_ref", doc! { "ref": rf, "id": id })?;
        Ok(())
    }

    pub fn obj_set_attr(&mut self, id: i32, key: &str, val: AttrValue) -> Result<()> {
        if key.contains('.') {
            return Err(Error::InvalidKey(key.to_string()));
        }
        let val = attr_to_bson(val);
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

    pub fn obj_del_dep(&mut self, id: i32, dep: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$pull": { "deps": dep } }, None)
            .unwrap();
        self.create_log("obj.del_dep", doc! { "dep": dep, "id": id })?;
        Ok(())
    }

    pub fn obj_del_sub(&mut self, id: i32, sub: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$pull": { "subs": sub } }, None)
            .unwrap();
        self.create_log("obj.del_sub", doc! { "sub": sub, "id": id })?;
        Ok(())
    }

    pub fn obj_del_ref(&mut self, id: i32, rf: i32) -> Result<()> {
        self.objs
            .find_one_and_update(doc! { "_id": id }, doc! { "$pull": { "refs": rf } }, None)
            .unwrap();
        self.create_log("obj.del_ref", doc! { "ref": rf, "id": id })?;
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
        let mut obj = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        // This mess is here because implementing `Deserialize` for Attr is complicated, and it's also only
        // used once
        Ok(Object {
            id,
            name: match obj.remove("name").unwrap() {
                Bson::String(s) => s,
                _ => panic!("Expected string for name!"),
            },
            typ: match obj.remove("type").unwrap() {
                Bson::String(s) => s,
                _ => panic!("Expected string for type!"),
            },
            desc: obj.remove("desc").map(|d| match d {
                Bson::String(d) => d,
                _ => panic!("Expected string for desc!"),
            }),
            deps: match obj.remove("deps") {
                Some(Bson::Array(a)) => a.into_iter().map(|d| d.as_i32().unwrap()).collect(),
                _ => Vec::new(),
            },
            refs: match obj.remove("refs") {
                Some(Bson::Array(a)) => a.into_iter().map(|d| d.as_i32().unwrap()).collect(),
                _ => Vec::new(),
            },
            subs: match obj.remove("subs") {
                Some(Bson::Array(a)) => a.into_iter().map(|d| d.as_i32().unwrap()).collect(),
                _ => Vec::new(),
            },
            attrs: match obj.remove("attrs") {
                Some(Bson::Document(d)) => doc_to_attr(d),
                _ => Attr::default(),
            },
        })
    }
}
