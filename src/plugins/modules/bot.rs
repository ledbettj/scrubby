use log::info;
use mlua::serde::LuaSerdeExt;
use mlua::{ExternalError, Lua, Table, Value as LuaValue, Variadic};

pub fn bot_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  // bot.register()
  let register = lua.create_function(|_: &Lua, (bot, arg): (Table, Table)| {
    let plugins: Table = bot.get("plugins")?;
    let name: String = arg.get("name")?;

    info!("[{name}] registered", name = name);
    plugins.set(name, arg)?;
    Ok(())
  })?;

  tbl.set("plugin", bot_new_plugin(lua)?)?;
  tbl.set("plugins", lua.create_table()?)?;
  tbl.set("register", register)?;

  Ok(tbl)
}

fn bot_new_plugin(lua: &Lua) -> mlua::Result<mlua::Function> {
  lua.create_function(|l: &Lua, (name, desc): (String, Option<String>)| {
    let tbl = l.create_table()?;

    // plugin.command()
    let command = l.create_function(|_: &Lua, (plg, cmd): (Table, Table)| {
      let cmds: Table = plg.get("commands")?;
      let name: String = cmd.get("name")?;
      cmds.set(name, cmd)?;
      Ok(())
    })?;

    // plugin.log()
    let log = l.create_function(|_: &Lua, (plug, vals): (Table, Variadic<String>)| {
      let text = vals.into_iter().collect::<String>();
      let name: String = plug.get("name")?;
      info!("[{name}] {text}", name = name, text = text);
      Ok(())
    })?;

    tbl.set("cache", plugin_new_cache(l, &tbl)?)?;
    tbl.set("name", name)?;
    tbl.set("description", desc)?;
    tbl.set("commands", l.create_table()?)?;
    tbl.set("command", command)?;
    tbl.set("log", log)?;

    Ok(tbl)
  })
}

fn plugin_new_cache<'a>(lua: &'a Lua, parent: &'a Table) -> mlua::Result<Table<'a>> {
  let cache = lua.create_table()?;
  cache.set("_ref", parent)?;
  cache.set("data", lua.create_table()?)?;

  // plugin.cache.get()
  let get = lua.create_function(|_: &Lua, (cache, key): (Table, String)| {
    cache
      .get::<&str, Table>("data")?
      .get::<&str, LuaValue>(&key)
  })?;

  // plugin.cache.set()
  let set = lua.create_function(|_: &Lua, (cache, key, value): (Table, String, LuaValue)| {
    cache.get::<&str, Table>("data")?.set(key, &value)
  })?;

  // plugin.cache.clear()
  let clear =
    lua.create_function(|_: &Lua, cache: Table| cache.get::<&str, Table>("data")?.clear())?;

  // plugin.cache.load()
  let load = lua.create_function(|nl: &Lua, cache: Table| {
    let name: String = cache
      .get::<&str, Table>("_ref")?
      .get::<&str, String>("name")?;
    let name = format!("{:x}.json", md5::compute(&name));
    let path = std::path::Path::new("./cache").join(name);
    if let Ok(buf) = std::fs::read_to_string(path) {
      let value: serde_json::Value = serde_json::from_str(&buf).map_err(|e| e.into_lua_err())?;
      let value = nl.to_value(&value)?;
      cache.set("data", value)?;
    }

    Ok(())
  })?;

  // plugin.cache.save()
  let save = lua.create_function(|_: &Lua, cache: Table| {
    let name: String = cache
      .get::<&str, Table>("_ref")?
      .get::<&str, String>("name")?;
    let name = format!("{:x}.json", md5::compute(&name));
    let path = std::path::Path::new("./cache").join(name);

    let data = cache.get::<&str, Table>("data")?;

    std::fs::write(
      path,
      &serde_json::to_string(&data).map_err(|e| e.into_lua_err())?,
    )
    .map_err(|e| e.into_lua_err())
  })?;

  cache.set("load", load)?;
  cache.set("save", save)?;
  cache.set("set", set)?;
  cache.set("get", get)?;
  cache.set("clear", clear)?;

  Ok(cache)
}
