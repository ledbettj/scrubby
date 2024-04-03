use mlua::UserData;
use serenity::model::channel::Message;

#[derive(Clone)]
pub struct LuaMessage(Message);

impl From<Message> for LuaMessage {
  fn from(m: Message) -> Self {
    Self(m)
  }
}

impl From<&Message> for LuaMessage {
  fn from(m: &Message) -> Self {
    Self(m.clone())
  }
}

impl UserData for LuaMessage {
  fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
    fields.add_field_method_get("id", |_, this| Ok(this.0.id.get()));
    fields.add_field_method_get("author", |_, this| Ok(this.0.author.name.clone()));
    fields.add_field_method_get("content", |_, this| Ok(this.0.content.clone()));
    fields.add_field_method_get("timestamp", |_, this| Ok(this.0.timestamp.unix_timestamp()));
    fields.add_field_method_get("tts", |_, this| Ok(this.0.tts));
  }

  fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(_: &mut M) {}
}
