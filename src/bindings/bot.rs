use mlua::serde::LuaSerdeExt;
use mlua::{Function as LuaFunction, Lua, Table, Value as LuaValue};

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

    let get = l.create_function(|nl: &Lua, (cache, key): (Table, String)| {
      if cache.contains_key(key.clone())? {
        cache.get(key)
      } else {
        // todo: fetch from disk
        let v = nl.to_value("{}")?;
        cache.set(key, &v)?;
        Ok(v)
      }
    })?;

    let set = l.create_function(|_: &Lua, (cache, key, value): (Table, String, LuaValue)| {
      // TODO: set from disk
      cache.set(key, &value)?;
      let data = serde_json::to_string(&value);
      Ok(())
    })?;

    let clear = l.create_function(|_: &Lua, (cache, key): (Table, String)| {
      // TODO: clear from disk
      cache.set(key, LuaValue::Nil)
    })?;

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
