use mlua::{ExternalError, Lua, Table, Value};

pub fn http_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  // http.get("https://aol.com", { headers = { Authorization = "..." } })
  let get = lua.create_function(|_, (url, opts): (String, Option<Table>)| {
    apply_headers(ureq::get(&url), &opts)
      .call()
      .map_err(|e| e.into_lua_err())?
      .into_string()
      .map_err(|e| e.into_lua_err())
  })?;

  // http.post("https://aol.com", "body", { headers = { ["Content-Type"] = "..." } })
  let post = lua.create_function(|_, (url, body, opts): (String, String, Option<Table>)| {
    apply_headers(ureq::post(&url), &opts)
      .send_string(&body)
      .map_err(|e| e.into_lua_err())?
      .into_string()
      .map_err(|e| e.into_lua_err())
  })?;

  tbl.set("get", get)?;
  tbl.set("post", post)?;
  Ok(tbl)
}

fn apply_headers(mut req: ureq::Request, opts: &Option<Table>) -> ureq::Request {
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
}
