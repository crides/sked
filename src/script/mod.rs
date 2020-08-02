mod lua;

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use lazy_static::lazy_static;
use rlua::prelude::*;

use crate::storage::Storage;

lazy_static! {
    static ref STORAGE: Mutex<Storage> = Mutex::new(Storage::new().unwrap());
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
                "add_log",
                ctx.create_function(
                    |ctx, (name, typ, map): (String, String, Option<HashMap<String, LuaValue>>)| {
                        let mut storage = STORAGE.lock().unwrap();
                        let id = storage
                            .create_log(&name, &typ)
                            .map_err(LuaError::external)?;
                        if let Some(map) = map {
                            if map.contains_key("desc") {
                                let desc = String::from_lua(map.get("desc").unwrap().clone(), ctx)?;
                                storage
                                    .log_set_desc(id, &desc)
                                    .map_err(LuaError::external)?
                            }
                            if map.contains_key("obj") {
                                let obj_ref =
                                    i32::from_lua(map.get("obj").unwrap().clone(), ctx)?;
                                storage
                                    .log_set_obj(id, obj_ref)
                                    .map_err(LuaError::external)?;
                            }
                        }
                        Ok(id)
                    },
                )?,
            )?;
            globals.set(
                "new_obj",
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
                        Ok(id)
                    },
                )?,
            )?;
            globals.set(
                "get_log",
                ctx.create_function(|_, id: i32| {
                    Ok(STORAGE.lock().unwrap().get_log(id).map_err(|e| {
                        LuaError::external(dbg!(e))
                    })?)
                })?,
            )?;
            globals.set(
                "get_obj",
                ctx.create_function(|_, id: i32| {
                    Ok(STORAGE
                        .lock()
                        .unwrap()
                        .get_obj(id)
                        .map_err(LuaError::external)?)
                })?,
            )?;
            Ok(())
        })
    }

    pub fn repl(&self) {
        self.lua.context(|ctx| {
            lua::repl(ctx);
        });
    }
}
