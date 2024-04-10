use mlua::{ExternalError, Lua, Table};

pub fn http_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  let get = lua.create_function(|_, url: String| {
    ureq::get(&url)
      .call()
      .map_err(|e| e.to_string().into_lua_err())?
      .into_string()
      .map_err(|e| e.to_string().into_lua_err())
  })?;
  tbl.set("get", get)?;
  Ok(tbl)
}
