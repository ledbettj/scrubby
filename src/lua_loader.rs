use mlua::IntoLua;

const LUA_PACKAGES: &[(&str, &str)] = &include!(concat!(env!("OUT_DIR"), "/lua_packages.rs"));

fn test(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
  let e = lua.create_table()?;
  e.set("test", "it is")?;
  Ok(e)
}

pub fn module_search(lua: &mlua::Lua) -> Result<mlua::Function, mlua::Error> {

  lua.create_function(|l, modname: String| {
    let target = format!("{}.lua", modname.replace(".", "/"));
    // check for native lua modules.
    if modname == "test2" {
      return l.create_function(move |nl, _ : mlua::Value|{
        test(nl)
      })?.into_lua(l);
    }

    // check for embedded .lua modules
    for &(pkg, contents) in LUA_PACKAGES {
      if pkg == &target {
        return l.create_function(move |nl, _ : mlua::Value|{
          nl.load(contents).eval::<mlua::Value>()
        })?.into_lua(l);
      }
    }

    "not found".into_lua(l)
  })
}
