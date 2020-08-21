mod lua;

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use bson::{Bson, Document};
use rlua::prelude::*;

use crate::storage::Storage;

struct LogRef<'func>(Arc<Mutex<Storage<'func>>>, i32);

impl<'func> LuaUserData for LogRef<'func> {
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

struct ObjRef<'func>(Arc<Mutex<Storage<'func>>>, i32);

impl<'func> LuaUserData for ObjRef<'func> {
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

struct APIState<'func>(Arc<Mutex<Storage<'func>>>);

impl<'func> LuaUserData for APIState<'func> {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method(
            "new_log",
            |_, state, (typ, map): (String, Option<HashMap<String, String>>)| {
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
        methods.add_method("get_log", |_, state, id| {
            Ok(LogRef(Arc::clone(&state.0), id))
        });
        methods.add_method(
            "new_obj",
            |_, state, (name, typ, map): (String, String, Option<HashMap<String, String>>)| {
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
        methods.add_method("get_obj", |_, state, id| {
            Ok(ObjRef(Arc::clone(&state.0), id))
        });

        methods.add_method_mut("add_event_handler", |_, state, (pat, func): (String, LuaFunction<'lua>)| {
            // FIXME See https://github.com/amethyst/rlua/issues/185
            state.0.lock().unwrap().add_lua(&pat, unsafe { std::mem::transmute(func) }).map_err(LuaError::external)?;
            Ok(())
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
            globals.set(
                "pprint",
                ctx.create_function(|ctx, lt| Ok(lua::pprint(&lt, &ctx)))?,
            )?;
            globals.set("repl", ctx.create_function(|ctx, ()| Ok(lua::repl(ctx)))?)?;
            globals.set(
                "readline",
                ctx.create_function(|_, p| Ok(lua::readline(p)))?,
            )?;
            globals.set(
                "sched",
                APIState(
                    Arc::new(Mutex::new(Storage::new().map_err(LuaError::external)?)),
                ),
            )?;
            Ok(())
        })
    }
}
