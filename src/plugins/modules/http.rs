use mlua::serde::LuaSerdeExt;
use mlua::{ExternalError, Lua, Table, Value};

pub fn http_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  // http.get("https://aol.com", { headers = { Authorization = "..." } })
  let get = lua.create_function(|l, (url, opts): (String, Option<Table>)| {
    let result = apply_headers(ureq::get(&url), &opts).call();
    let json = opts
      .and_then(|o| o.get::<&str, Option<bool>>("json").unwrap_or(None))
      .unwrap_or(false);

    build_return(l, result, json)
  })?;

  // http.post("https://aol.com", "body", { headers = { ["Content-Type"] = "..." } })
  let post = lua.create_function(|l, (url, body, opts): (String, String, Option<Table>)| {
    let result = apply_headers(ureq::post(&url), &opts).send_string(&body);
    let json = opts
      .and_then(|o| o.get::<&str, Option<bool>>("json").unwrap_or(None))
      .unwrap_or(false);

    build_return(l, result, json)
  })?;

  tbl.set("get", get)?;
  tbl.set("post", post)?;
  Ok(tbl)
}

fn build_return(
  lua: &Lua,
  response: Result<ureq::Response, ureq::Error>,
  json: bool,
) -> Result<mlua::Table, mlua::Error> {
  let ret = lua.create_table()?;
  let resp = match response {
    Ok(resp) => resp,
    Err(ureq::Error::Status(_, resp)) => resp,
    Err(e) => return Err(e.into_lua_err()),
  };

  ret.set("status", resp.status())?;
  let body = resp.into_string()?;

  if json {
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.into_lua_err())?;

    ret.set("json", lua.to_value(&value)?)?
  }

  ret.set("body", body)?;

  Ok(ret)
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
