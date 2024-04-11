use std::fs::File;
use std::io::Read;

use mlua::{Lua, Table};
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
    if let Err(e) = self.lua.load("math.randomseed(os.time())").exec() {
      println!("Failed to seed RNG: {}", e);
    }

    if !reload {
      self.init_module_loader()?;
    } else {
      self
        .lua
        .globals()
        .get::<&str, Table>("package")?
        .get::<&str, Table>("loaded")?
        .get::<&str, Table>("bot")?
        .get::<&str, Table>("plugins")?
        .clear()?;
    }
    self.load_files()?;
    Ok(())
  }

  pub fn dispatch_message(&self, m: &Message, _c: &Context) -> anyhow::Result<Vec<String>> {
    let lua_msg: LuaMessage = m.into();
    let plugins: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?
      .get::<&str, Table>("plugins")?;

    let mut replies = vec![];

    plugins.for_each::<String, mlua::Table>(|plugname, plugin| {
      let commands: mlua::Table = plugin.get("commands")?;
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
                  "lua error: dispatching command {} to {} failed: ```\n{}\n```",
                  cmd, plugname, e
                )),
              };
            }
          }
          Err(e) => replies.push(format!("Invalid command format `{}` : `{:?}`", cmd, e)),
        }
        Ok(())
      })?;
      Ok(())
    })?;

    Ok(replies)
  }

  pub fn process_ready_event(&self, r: &Ready) -> anyhow::Result<()> {
    let bot: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?;
    bot.set("name", r.user.name.clone())?;
    Ok(())
  }

  fn load_files(&self) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(&self.plugin_path)? {
      let entry = entry?;
      if let Some("lua") = entry.path().extension().and_then(|os| os.to_str()) {
        let file_name = entry.file_name();
        let mut file = File::open(entry.path())?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        self
          .lua
          .load(buf)
          .set_name(file_name.to_string_lossy())
          .exec()?;
      }
    }

    Ok(())
  }

  fn init_module_loader(&self) -> anyhow::Result<()> {
    let pkg: mlua::Table = self.lua.globals().get("package")?;
    let searchers: mlua::Table = pkg.get("searchers")?;

    let search_fn = lua_loader::module_search(&self.lua)?;
    searchers.clear()?;
    searchers.push(search_fn)?;

    Ok(())
  }
}
