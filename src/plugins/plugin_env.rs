use log::{debug, error, warn};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;

use mlua::serde::LuaSerdeExt;
use mlua::{IntoLua, Lua, Table};
use serenity::{
  builder::{CreateEmbed, CreateEmbedFooter, CreateMessage},
  model::gateway::Ready,
  prelude::Context,
};

use crate::claude::{Schema, Tool};
use crate::plugins::module_search;
use crate::plugins::modules::LuaClientCtx;

pub struct PluginEnv {
  path: String,
  lua: Lua,
}

impl PluginEnv {
  pub fn new<S: Into<String>>(path: S) -> Self {
    let lua = Lua::new();

    Self {
      lua,
      path: path.into(),
    }
  }

  pub fn load(&mut self, reload: bool) -> anyhow::Result<Vec<Tool>> {
    if let Err(e) = self.lua.load("math.randomseed(os.time())").exec() {
      warn!("Failed to seed RNG: {}", e);
    }

    let env_tbl = self.lua.create_table()?;
    let expose = env::var("LUA_EXPOSE_ENV").unwrap_or_default();
    expose
      .split(",")
      .map(|key| (key, env::var(key).ok()))
      .try_for_each(|(key, value)| {
        debug!("exposing env.{}", key);
        env_tbl.set(key.to_owned(), value.clone())
      })?;

    self.lua.globals().set("env", env_tbl)?;

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
    self.tools()
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
          error!("[{}] {}", plugname, e);
        }
      }
      Ok(())
    })?;

    Ok(())
  }

  fn load_files(&self) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(&self.path)? {
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
          error!("[{}] {}", plugname, e);
        }
      }
      Ok(())
    })?;

    Ok(())
  }

  fn init_module_loader(&self) -> anyhow::Result<()> {
    let pkg: mlua::Table = self.lua.globals().get("package")?;
    let searchers: mlua::Table = pkg.get("searchers")?;

    let search_fn = module_search(&self.lua)?;
    searchers.clear()?;
    searchers.push(search_fn)?;

    Ok(())
  }

  pub fn build_message_json(&self, json: serde_json::Value) -> anyhow::Result<CreateMessage> {
    match self.lua.to_value(&json)? {
      mlua::Value::Table(tbl) => self.build_message(&tbl),
      _ => Err(anyhow::format_err!("oops")),
    }
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

  pub fn invoke_tool(
    &self,
    name: &str,
    input: HashMap<String, String>,
  ) -> anyhow::Result<Option<String>> {
    if let Some((plugname, toolname)) = name.split_once("-") {
      let plugin = self
        .lua
        .globals()
        .get::<&str, Table>("package")?
        .get::<&str, Table>("loaded")?
        .get::<&str, Table>("bot")?
        .get::<&str, Table>("plugins")?
        .get::<&str, Table>(plugname)?;

      let cmd = plugin
        .get::<&str, Table>("commands")?
        .get::<&str, Table>(toolname)?;

      let func = cmd.get::<&str, mlua::Function>("method")?;

      let res: Option<String> = func.call((plugin, input.into_lua(&self.lua)?))?;
      Ok(res)
    } else {
      unreachable!();
    }
  }

  fn tools(&self) -> anyhow::Result<Vec<Tool>> {
    let plugins = self
      .lua
      .globals()
      .get::<&str, Table>("package")?
      .get::<&str, Table>("loaded")?
      .get::<&str, Table>("bot")?
      .get::<&str, Table>("plugins")?;

    let mut tools = vec![];

    plugins.for_each::<String, mlua::Table>(|plugname, plugin| {
      let commands: mlua::Table = plugin.get("commands")?;
      commands.for_each::<String, mlua::Table>(|cmdname, cmd| {
        let description = cmd.get("description")?;
        let input_schema: Option<Schema> = self
          .lua
          .from_value(cmd.get::<&str, mlua::Value>("schema")?)?;

        tools.push(Tool {
          name: format!("{}-{}", plugname, cmdname).into(),
          description,
          input_schema: input_schema.unwrap_or_default(),
        });
        Ok(())
      })
    })?;

    Ok(tools)
  }
}
