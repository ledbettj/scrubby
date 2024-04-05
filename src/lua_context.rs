use std::fs::File;
use std::io::Read;

use mlua::Lua;
use regex::Regex;
use serenity::{
  model::{channel::Message, gateway::Ready},
  prelude::Context,
};

use crate::lua_loader;
use crate::user_data::LuaMessage;

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
      let commands: mlua::Table = bot.get("commands")?;

      commands.clear()?;
      plugins.clear()?;
    }
    self.load_files()?;
    Ok(())
  }

  pub fn dispatch_message(&self, m: &Message, _c: &Context) -> anyhow::Result<Vec<String>> {
    let lua_msg: LuaMessage = m.into();
    let bot: mlua::Table = self.lua.globals().get("Bot")?;
    let mut replies = vec![];
    let commands: mlua::Table = bot.get("commands")?;

    commands.for_each::<String, mlua::Function>(|cmd, callback| {
      match Regex::new(&cmd) {
        Ok(r) => {
          if let Some(captures) = r.captures(&m.content) {
            let caps: Vec<String> = captures
              .iter()
              .filter_map(|c| c)
              .map(|c| c.as_str().to_owned())
              .collect();

            match callback.call((lua_msg.clone(), caps)) {
              Ok(None) => { /* no op */ }
              Ok(Some(s)) => replies.push(s),
              Err(e) => replies.push(format!(
                "lua error: dispatching command {} failed: {}",
                cmd, e
              )),
            };
          }
        }
        Err(e) => replies.push(format!("Invalid command format '{}' : {:?}'", cmd, e)),
      }
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
    let pkg: mlua::Table = self.lua.globals().get("package")?;
    let searchers: mlua::Table = pkg.get("searchers")?;

    let search_fn = lua_loader::module_search(&self.lua)?;
    searchers.clear()?;
    searchers.push(search_fn)?;

    let bot = self.lua.create_table()?;
    let plugin = self.lua.create_function(|l: &Lua, name: String| {
      let tbl = l.create_table()?;
      tbl.set("name", name)?;
      Ok(tbl)
    })?;

    let register = self.lua.create_function(|l: &Lua, tbl: mlua::Table| {
      let bot: mlua::Table = l.globals().get("Bot")?;
      let name: String = tbl.get("name")?;
      let plugins: mlua::Table = bot.get("plugins")?;

      println!("Plugin {} registered", &name);
      plugins.set(name, tbl)?;
      Ok(())
    })?;

    let command =
      self
        .lua
        .create_function(|l: &Lua, (cmd, callback): (String, mlua::Function)| {
          let bot: mlua::Table = l.globals().get("Bot")?;
          let cmds: mlua::Table = bot.get("commands")?;
          cmds.set(cmd, callback)?;
          Ok(())
        })?;

    bot.set("plugin", plugin)?;
    bot.set("register", register)?;
    bot.set("plugins", self.lua.create_table()?)?;
    bot.set("commands", self.lua.create_table()?)?;
    bot.set("command", command)?;

    self.lua.globals().set("Bot", bot)?;

    Ok(())
  }
}
