mod lua;

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use bson::{Document, Bson};
use lazy_static::lazy_static;
use rlua::prelude::*;

use crate::storage::Storage;

lazy_static! {
    static ref STORAGE: Mutex<Storage> = Mutex::new(Storage::new().unwrap());
}

#[derive(Clone, Copy)]
struct LogRef(i32);

impl LuaUserData for LogRef {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("set_attr", |_, r, (key, val): (String, String)| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .log_set_attr(r.0, &key, &val)
                .map_err(LuaError::external)?)
        });
        methods.add_method("get", |_, r, ()| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .get_log(r.0)
                .map_err(LuaError::external)?)
        });

        methods.add_meta_function(LuaMetaMethod::Index, |ctx, (r, field): (LogRef, String)| {
            let log = STORAGE
                .lock()
                .unwrap()
                .get_log(r.0)
                .map_err(LuaError::external)?;
            match field.as_str() {
                "type" => Ok(log.typ.to_lua(ctx)),
                "attrs" => Ok(log.attrs.to_lua(ctx)),
                "time" => Ok(log.time.to_string().to_lua(ctx)),
                "timestamp" => Ok(log.time.timestamp().to_lua(ctx)),
                "id" => Ok(r.0.to_lua(ctx)),
                _ => return Err(LuaError::external(anyhow!("Unknown field: {}", field))),
            }
        });
    }
}

#[derive(Clone, Copy)]
struct ObjRef(i32);

impl LuaUserData for ObjRef {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("set_desc", |_, r, desc: String| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_set_desc(r.0, &desc)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_sub", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_add_sub(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_ref", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_add_ref(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_dep", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_add_dep(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("set_attr", |_, r, (key, val): (String, String)| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_set_attr(r.0, &key, &val)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_sub", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_del_sub(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_ref", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_del_ref(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_dep", |_, r, obj| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_del_dep(r.0, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_attr", |_, r, key: String| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .obj_del_attr(r.0, &key)
                .map_err(LuaError::external)?)
        });
        methods.add_method("get", |_, r, ()| {
            Ok(STORAGE
                .lock()
                .unwrap()
                .get_obj(r.0)
                .map_err(LuaError::external)?)
        });

        methods.add_meta_function(LuaMetaMethod::Index, |ctx, (r, field): (ObjRef, String)| {
            let obj = STORAGE
                .lock()
                .unwrap()
                .get_obj(r.0)
                .map_err(LuaError::external)?;
            match field.as_str() {
                "name" => Ok(obj.name.to_lua(ctx)),
                "type" => Ok(obj.typ.to_lua(ctx)),
                "desc" => Ok(obj.desc.to_lua(ctx)),
                "subs" => Ok(obj.subs.to_lua(ctx)),
                "deps" => Ok(obj.deps.to_lua(ctx)),
                "refs" => Ok(obj.refs.to_lua(ctx)),
                "attrs" => Ok(obj.attrs.to_lua(ctx)),
                "id" => Ok(r.0.to_lua(ctx)),
                _ => return Err(LuaError::external(anyhow!("Unknown field: {}", field))),
            }
        });
    }
}

pub struct ScriptContext {
    lua: Lua,
}

impl ScriptContext {
    pub fn new() -> Result<Self> {
        Ok(Self { lua: Lua::new() })
    }

    pub fn init_user<P: AsRef<Path>>(&self, config_dir: P) -> Result<()> {
        let config_dir = config_dir.as_ref();
        let init_file = config_dir.join("init.lua");
        let code = read_to_string(&init_file)?;
        self.lua.context(|ctx| {
            let globals = ctx.globals();
            let package: LuaTable = globals.get("package").unwrap();
            let package_path: String = package.get("path").unwrap();
            let new_package_path = [
                &package_path,
                config_dir.join("?.lua").to_str().unwrap(),
                config_dir.join("?/init.lua").to_str().unwrap(),
            ]
            .join(";");
            package.set("path", new_package_path).unwrap();
            ctx.load(&code)
                .set_name(init_file.to_str().unwrap())
                .unwrap()
                .exec()
        })?;
        Ok(())
    }

    pub fn init_lib(&mut self) -> LuaResult<()> {
        self.lua.context(|ctx| {
            let globals = ctx.globals();
            let sched_mod = ctx.create_table()?;
            let obj_mod = ctx.create_table()?;
            let log_mod = ctx.create_table()?;
            globals.set(
                "pprint",
                ctx.create_function(|ctx, lt| Ok(lua::pprint(&lt, &ctx)))?,
            )?;
            globals.set("repl", ctx.create_function(|ctx, ()| Ok(lua::repl(ctx)))?)?;
            globals.set(
                "readline",
                ctx.create_function(|_, p| Ok(lua::readline(p)))?,
            )?;
            log_mod.set(
                "_new",
                ctx.create_function(
                    |_, (typ, map): (String, Option<HashMap<String, String>>)| {
                        let mut storage = STORAGE.lock().unwrap();
                        let attrs = map.map(|m| m.into_iter().map(|(k, v)| (k, Bson::String(v))).collect::<Document>()).unwrap_or_default();
                        let id = storage
                            .create_log(&typ, attrs)
                            .map_err(LuaError::external)?;
                        Ok(LogRef(id))
                    },
                )?,
            )?;
            log_mod.set("get", ctx.create_function(|_, id: i32| Ok(LogRef(id)))?)?;
            obj_mod.set(
                "new",
                ctx.create_function(
                    |ctx, (name, typ, map): (String, String, Option<HashMap<String, LuaValue>>)| {
                        let mut storage = STORAGE.lock().unwrap();
                        let id = storage
                            .create_obj(&name, &typ)
                            .map_err(LuaError::external)?;
                        if let Some(map) = map {
                            if map.contains_key("desc") {
                                let desc = String::from_lua(map.get("desc").unwrap().clone(), ctx)?;
                                storage
                                    .obj_set_desc(id, &desc)
                                    .map_err(LuaError::external)?;
                            }
                        }
                        Ok(ObjRef(id))
                    },
                )?,
            )?;
            obj_mod.set("get", ctx.create_function(|_, id| Ok(ObjRef(id)))?)?;

            sched_mod.set("obj", obj_mod)?;
            sched_mod.set("log", log_mod)?;
            globals.set("sched", sched_mod)?;
            Ok(())
        })
    }
}
