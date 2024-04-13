use colored::*;
use std::fs::File;
use std::io::Read;

use mlua::{ExternalError, Lua, Table};
use regex::Regex;
use serenity::{
  builder::{CreateEmbed, CreateEmbedFooter, CreateMessage},
  model::{channel::Message, gateway::Ready},
  prelude::Context,
};

use crate::user_data::LuaMessage;
use crate::{lua_loader, user_data::LuaClientCtx};

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
      println!("[{}] Failed to seed RNG: {}", "Error".red().bold(), e);
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

  pub fn dispatch_message(
    &self,
    m: &Message,
    _c: &Context,
  ) -> anyhow::Result<Vec<(Option<String>, Option<CreateMessage>)>> {
    let lua_msg: LuaMessage = m.into();
    let plugins: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?
      .get::<&str, Table>("plugins")?;

    let mut replies: Vec<(Option<String>, Option<CreateMessage>)> = vec![];

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
                Ok(Some(mlua::Value::String(s))) => {
                  replies.push((Some(s.to_str()?.to_owned()), None))
                }
                Ok(Some(mlua::Value::Table(t))) => {
                  let msg = self.build_message(&t).map_err(|e| e.into_lua_err())?;
                  replies.push((None, Some(msg)));
                }
                Ok(_) => {}
                Err(e) => replies.push((
                  Some(format!(
                    "lua error: dispatching command {} to {} failed: ```\n{}\n```",
                    cmd, plugname, e
                  )),
                  None,
                )),
              };
            }
          }
          Err(e) => replies.push((
            Some(format!("Invalid command format `{}` : `{:?}`", cmd, e)),
            None,
          )),
        }
        Ok(())
      })?;
      Ok(())
    })?;

    Ok(replies)
  }

  pub fn process_ready_event(&self, r: &Ready, ctx: &Context) -> anyhow::Result<()> {
    let bot: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?;
    bot.set("name", r.user.name.clone())?;

    let plugins: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?
      .get::<&str, Table>("plugins")?;

    plugins.for_each::<String, mlua::Table>(|plugname, plugin| {
      if let mlua::Value::Function(ready) = plugin.get::<&str, mlua::Value>("ready")? {
        if let Err(e) = ready.call::<(Table, LuaClientCtx), ()>((plugin, ctx.into())) {
          println!(
            "[{}] [{}] {}",
            plugname.cyan().bold(),
            "Error".red().bold(),
            e
          );
        }
      }
      Ok(())
    })?;

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

  pub fn process_tick_event(&self, ctx: &Context) -> anyhow::Result<()> {
    let plugins: mlua::Table = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?
      .get::<&str, Table>("plugins")?;

    plugins.for_each::<String, mlua::Table>(|plugname, plugin| {
      if let mlua::Value::Function(tick) = plugin.get::<&str, mlua::Value>("tick")? {
        if let Err(e) = tick.call::<(Table, LuaClientCtx), ()>((plugin, ctx.into())) {
          println!(
            "[{}] [{}] {}",
            plugname.cyan().bold(),
            "Error".red().bold(),
            e
          );
        }
      }
      Ok(())
    })?;

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

  fn build_message(&self, table: &Table) -> anyhow::Result<CreateMessage> {
    let mut builder = CreateMessage::new();
    if let Some(content) = table.get::<&str, Option<String>>("content")? {
      builder = builder.content(content);
    }

    if let Some(embed) = table.get::<&str, Option<Table>>("embed")? {
      let mut e = CreateEmbed::new();
      if let Some(s) = embed.get::<&str, Option<String>>("title")? {
        e = e.title(s);
      }
      if let Some(s) = embed.get::<&str, Option<String>>("thumbnail")? {
        e = e.thumbnail(s);
      }
      if let Some(s) = embed.get::<&str, Option<String>>("description")? {
        e = e.description(s);
      }
      if let Some(s) = embed.get::<&str, Option<String>>("footer")? {
        e = e.footer(CreateEmbedFooter::new(s));
      }

      if let Some(f) = embed.get::<&str, Option<Table>>("fields")? {
        for pair in f.pairs::<isize, Table>() {
          let (_, row) = pair?;
          e = e.field(
            row.get::<isize, String>(1)?,
            row.get::<isize, String>(2)?,
            row.get::<isize, bool>(3)?,
          );
        }
      }

      builder = builder.embed(e);
    }

    Ok(builder)
  }
}
