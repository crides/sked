mod lua;

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use bson::{Bson, Document};
use rlua::prelude::*;
use regex::Regex;

use crate::storage::{Storage, LuaRegex};

struct LogRef(Arc<Mutex<Storage>>, i32);

impl LuaUserData for LogRef {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("set_attr", |_, rf, (key, val): (String, String)| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .log_set_attr(rf.1, &key, &val)
                .map_err(LuaError::external)?)
        });
        methods.add_method("get", |_, rf, ()| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .get_log(rf.1)
                .map_err(LuaError::external)?)
        });

        methods.add_meta_method(LuaMetaMethod::Index, |ctx, rf, field: String| {
            let log =
                rf.0.lock()
                    .unwrap()
                    .get_log(rf.1)
                    .map_err(LuaError::external)?;
            match field.as_str() {
                "type" => Ok(log.typ.to_lua(ctx)),
                "attrs" => Ok(log.attrs.to_lua(ctx)),
                "time" => Ok(log.time.to_string().to_lua(ctx)),
                "timestamp" => Ok(log.time.timestamp().to_lua(ctx)),
                "id" => Ok(rf.1.to_lua(ctx)),
                _ => return Err(LuaError::external(anyhow!("Unknown field: {}", field))),
            }
        });
    }
}

struct ObjRef(Arc<Mutex<Storage>>, i32);

impl LuaUserData for ObjRef {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("set_desc", |_, rf, desc: String| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_set_desc(rf.1, &desc)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_sub", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_add_sub(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_ref", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_add_ref(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("add_dep", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_add_dep(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("set_attr", |_, rf, (key, val): (String, String)| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_set_attr(rf.1, &key, &val)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_sub", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_del_sub(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_ref", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_del_ref(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_dep", |_, rf, obj| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_del_dep(rf.1, obj)
                .map_err(LuaError::external)?)
        });
        methods.add_method("del_attr", |_, rf, key: String| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .obj_del_attr(rf.1, &key)
                .map_err(LuaError::external)?)
        });
        methods.add_method("get", |_, rf, ()| {
            Ok(rf
                .0
                .lock()
                .unwrap()
                .get_obj(rf.1)
                .map_err(LuaError::external)?)
        });

        methods.add_meta_method(LuaMetaMethod::Index, |ctx, rf, field: String| {
            let obj =
                rf.0.lock()
                    .unwrap()
                    .get_obj(rf.1)
                    .map_err(LuaError::external)?;
            match field.as_str() {
                "name" => Ok(obj.name.to_lua(ctx)),
                "type" => Ok(obj.typ.to_lua(ctx)),
                "desc" => Ok(obj.desc.to_lua(ctx)),
                "subs" => Ok(obj.subs.to_lua(ctx)),
                "deps" => Ok(obj.deps.to_lua(ctx)),
                "refs" => Ok(obj.refs.to_lua(ctx)),
                "attrs" => Ok(obj.attrs.to_lua(ctx)),
                "id" => Ok(rf.1.to_lua(ctx)),
                _ => return Err(LuaError::external(anyhow!("Unknown field: {}", field))),
            }
        });
    }
}

#[derive(Clone)]
struct APIState(Arc<Mutex<Storage>>);

impl LuaUserData for APIState {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method(
            "new_log",
            |_,
             state,
             (typ, map): (String, Option<HashMap<String, String>>)|
             -> LuaResult<LogRef> {
                let attrs = map
                    .map(|m| {
                        m.into_iter()
                            .map(|(k, v)| (k, Bson::String(v)))
                            .collect::<Document>()
                    })
                    .unwrap_or_default();
                let id = state
                    .0
                    .lock()
                    .unwrap()
                    .create_log(&typ, attrs)
                    .map_err(LuaError::external)?;
                Ok(LogRef(Arc::clone(&state.0), id))
            },
        );
        methods.add_method("get_log", |_, state, id| -> LuaResult<LogRef> {
            Ok(LogRef(Arc::clone(&state.0), id))
        });
        methods.add_method(
            "new_obj",
            |_,
             state,
             (name, typ, map): (String, String, Option<HashMap<String, String>>)|
             -> LuaResult<ObjRef> {
                let mut storage = state.0.lock().unwrap();
                let id = storage
                    .create_obj(&name, &typ)
                    .map_err(LuaError::external)?;
                if let Some(map) = map {
                    if map.contains_key("desc") {
                        storage
                            .obj_set_desc(id, map.get("desc").unwrap())
                            .map_err(LuaError::external)?;
                    }
                }
                Ok(ObjRef(Arc::clone(&state.0), id))
            },
        );
        methods.add_method("get_obj", |_, state, id| -> LuaResult<ObjRef> {
            Ok(ObjRef(Arc::clone(&state.0), id))
        });
        methods.add_method_mut(
            "add_handler",
            |ctx, _state, (pat, func): (String, LuaFunction<'lua>)| {
                let entry = ctx.create_table()?;
                entry.set(1, LuaRegex(Regex::new(&pat).map_err(LuaError::external)?))?;
                entry.set(2, func)?;
                let handlers: LuaTable = ctx.named_registry_value("handlers")?;
                handlers.set(handlers.len().unwrap() + 1, entry)?;
                ctx.set_named_registry_value("handlers", handlers)?;
                Ok(())
            },
        );
    }
}

pub fn run_script(config_dir: &Path) -> Result<()> {
    let state = APIState(Arc::new(Mutex::new(Storage::new())));
    let storage = state.0.lock().unwrap();
    storage.lua.context(|ctx| -> LuaResult<()> {
        let globals = ctx.globals();
        globals.set(
            "pprint",
            ctx.create_function(|ctx, lt| Ok(lua::pprint(&lt, &ctx)))?,
        )?;
        globals.set("repl", ctx.create_function(|ctx, ()| Ok(lua::repl(ctx)))?)?;
        globals.set(
            "readline",
            ctx.create_function(|_, p| Ok(lua::readline(p)))?,
        )?;
        globals.set("sched", APIState(Arc::clone(&state.0)))?;
        ctx.set_named_registry_value("handlers", ctx.create_table()?)?;
        Ok(())
    })?;
    let init_file = config_dir.join("init.lua");
    let code = read_to_string(&init_file)?;
    storage.lua.context(|ctx| {
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
