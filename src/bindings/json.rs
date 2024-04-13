use mlua::serde::LuaSerdeExt;
use mlua::{ExternalError, Lua, Table, Value};

pub fn json_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  // json.serialize({ foo = 3 })
  let serialize = lua.create_function(|_: &Lua, value: Value| {
    serde_json::to_string(&value).map_err(|e| e.into_lua_err())
  })?;

  // json.deserialize("{\"foo\":3}")
  let deserialize = lua.create_function(|l: &Lua, buf: String| {
    let value: serde_json::Value = serde_json::from_str(&buf).map_err(|e| e.into_lua_err())?;
    l.to_value(&value)
  })?;

  tbl.set("encode", serialize)?;
  tbl.set("decode", deserialize)?;

  Ok(tbl)
}
