use mlua::{IntoLua, Lua};

use crate::bindings::{bot::bot_loader, http::http_loader};

const LUA_PACKAGES: &[(&str, &str)] = &include!(concat!(env!("OUT_DIR"), "/lua_packages.rs"));
const LUA_MODULES: [(
  &str,
  for<'a> fn(&'a Lua) -> Result<mlua::Table<'a>, mlua::Error>,
); 2] = [("http", http_loader), ("bot", bot_loader)];

pub fn module_search(lua: &Lua) -> Result<mlua::Function, mlua::Error> {
  lua.create_function(|l, modname: String| {
    // check for native lua modules.
    for &(pkg, loader) in &LUA_MODULES {
      if modname == pkg {
        return l
          .create_function(move |nl, _: mlua::Value| loader(nl))?
          .into_lua(l);
      }
    }

    let target = format!("{}.lua", modname.replace(".", "/"));
    // check for embedded .lua modules
    for &(pkg, contents) in LUA_PACKAGES {
      if pkg == &target {
        return l
          .create_function(move |nl, _: mlua::Value| nl.load(contents).eval::<mlua::Value>())?
          .into_lua(l);
      }
    }

    "not found".into_lua(l)
  })
}
