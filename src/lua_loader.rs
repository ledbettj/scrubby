const LUA_PACKAGES: &[(&str, &str)] = &include!(concat!(env!("OUT_DIR"), "/lua_packages.rs"));

pub fn module_search(lua: &mlua::Lua) -> Result<mlua::Function, mlua::Error> {
  lua.create_function(|l, modname: String| {
    let target = format!("{}.lua", modname.replace(".", "/"));
    for &(pkg, contents) in LUA_PACKAGES {
      if pkg == &target {
        return l.load(contents).eval();
      }
    }

    Ok(mlua::Value::Nil)
  })
}
