use std::collections::HashMap;

use chrono::Local;
use redis::{Client, Commands, Connection, RedisResult};

use crate::storage::*;

pub struct Storage(Connection);

pub fn log_id(id: usize) -> String {
    format!("logs:{}", id)
}

pub fn obj_id(id: ObjectRef) -> String {
    format!("objs:{}", id)
}

impl Storage {
    pub fn new() -> RedisResult<Storage> {
        Ok(Storage(
            Client::open("redis://localhost")?.get_connection()?,
        ))
    }

    pub fn create_log(&mut self, name: &str, typ: &str) -> RedisResult<usize> {
        let id: usize = self.0.incr("log_id_next", 1)?;
        let time = Local::now().timestamp();
        self.0.hset_multiple(
            log_id(id),
            &[("name", name), ("type", typ), ("time", &time.to_string())],
        )?;
        Ok(id)
    }

    pub fn log_set_desc(&mut self, id: usize, desc: &str) -> RedisResult<()> {
        self.0.hset_nx(log_id(id), "desc", desc)
    }

    pub fn log_set_obj(&mut self, id: usize, obj: ObjectRef) -> RedisResult<()> {
        self.0.hset_nx(log_id(id), "obj", obj)
    }

    pub fn get_log(&mut self, id: usize) -> RedisResult<Log> {
        let id = log_id(id);
        let name = self.0.hget(&id, "name")?;
        let typ = self.0.hget(&id, "type")?;
        let time = self.0.hget(&id, "time")?;
        let desc = self.0.hget(&id, "desc")?;
        let obj = self.0.hget(&id, "obj")?;
        Ok(Log {
            name,
            typ,
            time,
            desc,
            obj,
        })
    }

    pub fn create_obj(&mut self, name: &str, typ: &str) -> RedisResult<ObjectRef> {
        let id: usize = self.0.incr("obj_id_next", 1)?;
        self.0
            .hset_multiple(obj_id(id), &[("name", name), ("type", typ)])?;
        Ok(id)
    }

    pub fn obj_set_desc(&mut self, id: usize, desc: &str) -> RedisResult<()> {
        self.0.hset_nx(obj_id(id), "desc", desc)
    }

    pub fn obj_add_dep(&mut self, id: ObjectRef, dep: ObjectRef) -> RedisResult<()> {
        self.0.rpush(format!("objs:{}:deps", id), dep)
    }

    pub fn obj_add_sub(&mut self, id: ObjectRef, sub: ObjectRef) -> RedisResult<()> {
        self.0.rpush(format!("objs:{}:subs", id), sub)
    }

    pub fn obj_add_ref(&mut self, id: ObjectRef, rf: ObjectRef) -> RedisResult<()> {
        self.0.rpush(format!("objs:{}:refs", id), rf)
    }

    pub fn obj_set_attr(&mut self, id: ObjectRef, name: &str, val: &str) -> RedisResult<()> {
        self.0.hset(format!("objs:{}:attrs", id), name, val)
    }

    pub fn get_obj(&mut self, id: ObjectRef) -> RedisResult<Object> {
        let id = obj_id(id);
        let name = self.0.hget(&id, "name")?;
        let typ = self.0.hget(&id, "type")?;
        let desc = self.0.hget(&id, "desc")?;
        let deps = self.0.lrange(format!("{}:deps", id), 0, -1)?;
        let subs = self.0.lrange(format!("{}:subs", id), 0, -1)?;
        let refs = self.0.lrange(format!("{}:refs", id), 0, -1)?;
        let attrs = self.0.hgetall(format!("{}:attrs", id))?;
        Ok(Object {
            name,
            typ,
            desc,
            deps,
            subs,
            refs,
            attrs,
        })
    }
}
