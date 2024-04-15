use mlua::{IntoLua, Lua, UserData};
use serenity::gateway::ActivityData;
use serenity::model::channel::Message;
use serenity::prelude::Context;

mod bot;
mod http;
mod json;

use bot::bot_loader;
use http::http_loader;
use json::json_loader;

const LUA_PACKAGES: &[(&str, &str)] = &include!(concat!(env!("OUT_DIR"), "/lua_packages.rs"));
const LUA_MODULES: [(
  &str,
  for<'a> fn(&'a Lua) -> Result<mlua::Table<'a>, mlua::Error>,
); 3] = [
  ("bot", bot_loader),
  ("http", http_loader),
  ("json", json_loader),
];

pub fn module_search(lua: &Lua) -> Result<mlua::Function, mlua::Error> {
  lua.create_function(|l, modname: String| {
    // check for native lua modules.
    for &(pkg, loader) in &LUA_MODULES {
      if modname == pkg {
        return l
          .create_function(move |nl, _: mlua::Value| loader(nl))?
          .into_lua(l);
      }
    }

    let target = format!("{}.lua", modname.replace(".", "/"));
    // check for embedded .lua modules
    for &(pkg, contents) in LUA_PACKAGES {
      if pkg == &target {
        return l
          .create_function(move |nl, _: mlua::Value| nl.load(contents).eval::<mlua::Value>())?
          .into_lua(l);
      }
    }

    "not found".into_lua(l)
  })
}

#[derive(Clone)]
pub struct LuaMessage(Message);
pub struct LuaClientCtx(Context);

impl From<Message> for LuaMessage {
  fn from(m: Message) -> Self {
    Self(m)
  }
}

impl From<&Message> for LuaMessage {
  fn from(m: &Message) -> Self {
    Self(m.clone())
  }
}

impl UserData for LuaMessage {
  fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
    fields.add_field_method_get("id", |_, this| Ok(this.0.id.get()));
    fields.add_field_method_get("author", |_, this| Ok(this.0.author.name.clone()));
    fields.add_field_method_get("content", |_, this| Ok(this.0.content.clone()));
    fields.add_field_method_get("timestamp", |_, this| Ok(this.0.timestamp.unix_timestamp()));
    fields.add_field_method_get("tts", |_, this| Ok(this.0.tts));
    fields.add_field_method_get("channel_id", |_, this| {
      Ok(Into::<u64>::into(this.0.channel_id))
    });
  }

  fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(_: &mut M) {}
}

impl From<Context> for LuaClientCtx {
  fn from(c: Context) -> Self {
    Self(c)
  }
}

impl From<&Context> for LuaClientCtx {
  fn from(c: &Context) -> Self {
    Self(c.clone())
  }
}

impl UserData for LuaClientCtx {
  fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
    methods.add_method("online", |_, this, ()| Ok(this.0.online()));
    methods.add_method("idle", |_, this, ()| Ok(this.0.idle()));
    methods.add_method("dnd", |_, this, ()| Ok(this.0.dnd()));
    methods.add_method("set_activity", |_, this, playing: Option<String>| {
      match playing {
        Some(s) => this.0.set_activity(Some(ActivityData::playing(s))),
        None => this.0.set_activity(None),
      };
      Ok(())
    });
  }
}
