use mlua::{ExternalError, Lua, Table, Value};

pub fn http_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  let get = lua.create_function(|_, (url, opts): (String, Option<Table>)| {
    let mut req = ureq::get(&url);
    if let Some(tbl) = opts {
      if let Ok(Value::Table(hdrs)) = tbl.get("headers") {
        for pair in hdrs.pairs::<String, String>() {
          if let Ok((k, v)) = pair {
            req = req.set(&k, &v);
          }
        }
      };
    }

    req
      .call()
      .map_err(|e| e.into_lua_err())?
      .into_string()
      .map_err(|e| e.into_lua_err())
  })?;

  let post = lua.create_function(|_, (url, body, opts): (String, String, Option<Table>)| {
    let mut req = ureq::post(&url);
    if let Some(tbl) = opts {
      if let Ok(Value::Table(hdrs)) = tbl.get("headers") {
        for pair in hdrs.pairs::<String, String>() {
          if let Ok((k, v)) = pair {
            req = req.set(&k, &v);
          }
        }
      };
    }

    let resp = req
      .send_string(&body)
      .map_err(|e| e.into_lua_err())?
      .into_string()
      .map_err(|e| e.into_lua_err());

    println!("{:?}", resp);
    resp
  })?;

  tbl.set("get", get)?;
  tbl.set("post", post)?;
  Ok(tbl)
}
