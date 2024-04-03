use std::fs::File;
use std::io::Read;

use mlua::Lua;
use serenity::{
  model::{channel::Message, gateway::Ready},
  prelude::Context,
};

#[derive(Clone)]
struct LuaMessage(Message);

impl<'lua> mlua::IntoLua<'lua> for LuaMessage {
  fn into_lua(self, lua: &'lua Lua) -> mlua::Result<mlua::Value<'lua>> {
    let msg = self.0;
    let tbl = lua.create_table()?;
    tbl.set("id", msg.id.get())?;
    tbl.set("author", msg.author.name)?;
    tbl.set("content", msg.content)?;

    Ok(mlua::Value::Table(tbl))
  }
}

pub struct LuaContext {
  plugin_path: String,
  lua: Lua,
}

impl LuaContext {
  pub fn new<S: Into<String>>(plugin_path: S) -> Self {
    let lua = Lua::new();

    Self {
      lua,
      plugin_path: plugin_path.into(),
    }
  }

  pub fn load_plugins(&mut self, reload: bool) -> anyhow::Result<()> {
    if !reload {
      self.init_tables()?;
    } else {
      let bot: mlua::Table = self.lua.globals().get("Bot")?;
      let plugins: mlua::Table = bot.get("plugins")?;
      plugins.clear()?;
    }
    self.load_files()?;
    Ok(())
  }

  pub fn dispatch_message(&self, m: &Message, _c: &Context) -> anyhow::Result<Vec<String>> {
    let lua_msg = LuaMessage(m.clone());
    let bot: mlua::Table = self.lua.globals().get("Bot")?;
    let plugins: mlua::Table = bot.get("plugins")?;
    let mut replies = vec![];

    plugins.for_each::<String, mlua::Table>(|name, plugin| {
      let f: mlua::Function = plugin.get("on_message")?;
      match f.call::<LuaMessage, Option<String>>(lua_msg.clone()) {
        Ok(None) => { /* no op */ }
        Ok(Some(s)) => replies.push(s),
        Err(e) => replies.push(format!(
          "lua error: dispatching event to {} failed: {}",
          name, e
        )),
      };
      Ok(())
    })?;

    Ok(replies)
  }

  pub fn process_ready_event(&self, r: &Ready) -> anyhow::Result<()> {
    let bot: mlua::Table = self.lua.globals().get("Bot")?;
    bot.set("name", r.user.name.clone())?;
    Ok(())
  }

  fn load_files(&self) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(&self.plugin_path)? {
      let entry = entry?;
      if let Some("lua") = entry.path().extension().and_then(|os| os.to_str()) {
        let mut file = File::open(entry.path())?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        self.lua.load(buf).exec()?;
      }
    }

    Ok(())
  }

  fn init_tables(&self) -> anyhow::Result<()> {
    let bot = self.lua.create_table()?;
    let plugin = self.lua.create_function(|l: &Lua, name: String| {
      let tbl = l.create_table()?;
      let def = l.create_function(|_, ()| Ok(()))?;
      tbl.set("name", name)?;
      tbl.set("on_message", def)?;
      Ok(tbl)
    })?;

    let plugins = self.lua.create_table()?;
    let register = self.lua.create_function(|l: &Lua, tbl: mlua::Table| {
      let bot: mlua::Table = l.globals().get("Bot")?;
      let name: String = tbl.get("name")?;
      let plugins: mlua::Table = bot.get("plugins")?;

      println!("Plugin {} registered", &name);
      plugins.set(name, tbl)?;
      Ok(())
    })?;

    bot.set("plugin", plugin)?;
    bot.set("plugins", plugins)?;
    bot.set("register", register)?;

    self.lua.globals().set("Bot", bot)?;

    Ok(())
  }
}
