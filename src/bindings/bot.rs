use mlua::serde::LuaSerdeExt;
use mlua::{ExternalError, Function as LuaFunction, Lua, Table, Value as LuaValue};

pub fn bot_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;
  let plugin = lua.create_function(|l: &Lua, name: String| {
    let tbl = l.create_table()?;
    let command = l.create_function(
      |_: &Lua, (plg, cmd, callback): (Table, String, LuaFunction)| {
        let cmds: Table = plg.get("commands")?;
        cmds.set(cmd, callback)?;
        Ok(())
      },
    )?;

    let cache = l.create_table()?;
    cache.set("_ref", &tbl)?;
    cache.set("data", l.create_table()?)?;

    let get = l.create_function(|_: &Lua, (cache, key): (Table, String)| {
      cache
        .get::<&str, Table>("data")?
        .get::<&str, LuaValue>(&key)
    })?;

    let set = l.create_function(|_: &Lua, (cache, key, value): (Table, String, LuaValue)| {
      cache.get::<&str, Table>("data")?.set(key, &value)
    })?;

    let clear =
      l.create_function(|_: &Lua, cache: Table| cache.get::<&str, Table>("data")?.clear())?;

    let load = l.create_function(|nl: &Lua, cache: Table| {
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

    let save = l.create_function(|_: &Lua, cache: Table| {
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

    tbl.set("cache", cache)?;
    tbl.set("name", name)?;
    tbl.set("commands", l.create_table()?)?;
    tbl.set("command", command)?;

    Ok(tbl)
  })?;

  let register = lua.create_function(|_: &Lua, (bot, arg): (Table, Table)| {
    let plugins: Table = bot.get("plugins")?;
    let name: String = arg.get("name")?;

    println!("Plugin {} registered", &name);
    plugins.set(name, arg)?;
    Ok(())
  })?;

  tbl.set("plugin", plugin)?;
  tbl.set("plugins", lua.create_table()?)?;
  tbl.set("register", register)?;

  Ok(tbl)
}
