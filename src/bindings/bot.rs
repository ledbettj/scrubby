use mlua::{Lua, Table};

pub fn bot_loader(lua: &Lua) -> mlua::Result<Table> {
  let tbl = lua.create_table()?;

  let plugin = lua.create_function(|l: &Lua, name: String| {
    let tbl = l.create_table()?;
    tbl.set("name", name)?;
    Ok(tbl)
  })?;

  let command = lua.create_function(
    |_: &Lua, (bot, cmd, callback): (Table, String, mlua::Function)| {
      let cmds: Table = bot.get("commands")?;
      cmds.set(cmd, callback)?;
      Ok(())
    },
  )?;

  let register = lua.create_function(|_: &Lua, (bot, arg): (Table, Table)| {
    let plugins: Table = bot.get("plugins")?;
    let name: String = arg.get("name")?;

    println!("Plugin {} registered", &name);
    plugins.set(name, arg)?;
    Ok(())
  })?;

  tbl.set("plugin", plugin)?;
  tbl.set("plugins", lua.create_table()?)?;
  tbl.set("commands", lua.create_table()?)?;
  tbl.set("command", command)?;
  tbl.set("register", register)?;

  Ok(tbl)
}
