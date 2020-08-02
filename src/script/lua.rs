//! All the Lua helper functions/APIs that doesn't have anything to do with the core Logging and
//! Event APIs.

use rlua::prelude::*;
use rlua_serde::*;
use rustyline::{Config, Editor};

use crate::storage::{Log, Object};

macro_rules! impl_to_from_lua {
    { $( $x:ty )* } => {$(
        impl<'lua> FromLua<'lua> for $x {
            fn from_lua(value: LuaValue<'lua>, _ctx: LuaContext<'lua>) -> LuaResult<Self> {
                from_value(value)
            }
        }

        impl<'lua> ToLua<'lua> for $x {
            fn to_lua(self, ctx: LuaContext<'lua>) -> LuaResult<LuaValue<'lua>> {
                to_value(ctx, self)
            }
        }
    )*};
}

impl_to_from_lua! { Log Object }

pub fn repl(ctx: LuaContext) {
    let mut editor = Editor::<()>::with_config(Config::builder().tab_stop(4).build());
    loop {
        let mut prompt = "> ";
        let mut line = String::new();
        loop {
            match editor.readline(prompt) {
                Ok(input) => line.push_str(&input),
                Err(_) => return,
            }

            match ctx.load(&line).eval::<LuaMultiValue>() {
                Ok(values) => {
                    editor.add_history_entry(line);
                    println!(
                        "{}",
                        values
                            .iter()
                            .map(|value| format_value(value, &ctx))
                            .collect::<Vec<_>>()
                            .join("\t")
                    );
                    break;
                }
                Err(LuaError::SyntaxError {
                    incomplete_input: true,
                    ..
                }) => {
                    // continue reading input and append it to `line`
                    line.push_str("\n"); // separate input lines
                    prompt = ">> ";
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    if let LuaError::CallbackError { cause: c, .. } = e {
                        println!("Caused by: {}", c);
                    }
                    break;
                }
            }
        }
    }
}

pub fn format_value<'lua>(v: &LuaValue<'lua>, ctx: &LuaContext<'lua>) -> String {
    let mut s = String::new();
    let mut formatter = LuaFormatter::new(2);
    formatter.format_value(ctx, v, &mut s);
    s
}

pub fn pprint<'lua>(v: &LuaValue<'lua>, ctx: &LuaContext<'lua>) {
    println!("{}", format_value(v, ctx));
}

pub fn readline(prompt: String) -> LuaResult<String> {
    let mut editor = Editor::<()>::new();
    editor.readline(&prompt).map_err(LuaError::external)
}

struct LuaFormatter {
    indent: usize,
    has_value: bool,
    indent_size: usize,
}

impl LuaFormatter {
    fn new(i: usize) -> Self {
        Self {
            indent: 0,
            has_value: false,
            indent_size: i,
        }
    }

    fn format_value(&mut self, ctx: &LuaContext<'_>, v: &LuaValue<'_>, s: &mut String) {
        match v {
            LuaValue::Nil => s.push_str("nil"),
            LuaValue::Boolean(b) => s.push_str(&b.to_string()),
            LuaValue::Integer(i) => s.push_str(&i.to_string()),
            LuaValue::Number(f) => s.push_str(&f.to_string()),
            LuaValue::String(_s) => match _s.to_str() {
                Ok(_s) => s.push_str(_s),
                Err(_) => s.push_str(&format!("{:?}", _s)),
            },
            LuaValue::LightUserData(d) => s.push_str(&format!("{:?}", d)),
            LuaValue::UserData(d) => s.push_str(&format!("{:?}", d)),
            LuaValue::Function(f) => s.push_str(&format!("{:?}", f)),
            LuaValue::Thread(t) => s.push_str(&format!("{:?}", t)),
            LuaValue::Table(t) => {
                s.push_str("{");
                self.has_value = false;
                self.indent += 1;
                for pair in t.clone().pairs::<LuaValue, LuaValue>() {
                    let (key, val) = pair.unwrap();
                    s.push('\n');
                    s.push_str(&" ".repeat(self.indent * self.indent_size));
                    self.format_value(ctx, &key, s);
                    s.push_str(" = ");
                    self.format_value(ctx, &val, s);
                    self.has_value = true;
                    s.push(',');
                }
                self.indent -= 1;
                if self.has_value {
                    s.push('\n');
                    s.push_str(&" ".repeat(self.indent * self.indent_size));
                }
                s.push_str("}");
            }
            LuaValue::Error(e) => s.push_str(&format!("{:?}", e)),
        }
    }
}
