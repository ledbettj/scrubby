use handlebars::Handlebars;
use log::info;
use rusqlite::{params, Connection, Result as SqlResult};
use serde_json;
use std::collections::HashMap;
use std::path::Path;

use crate::PROMPT_TEMPLATE;

const DEFAULT_PERSONALITY: &'static str = "friendly and enthusiastic. Feel free to use some good-natured insults or jabs. You can use some emoji sparingly";

#[derive(Debug)]
pub struct GuildConfig {
  id: u64,
  guild_id: u64,
  config: serde_json::Value,
}

impl GuildConfig {
  pub fn system(&self) -> String {
    let mut tmpl = Handlebars::new();
    let mut map = HashMap::new();

    map.insert("personality".to_owned(), DEFAULT_PERSONALITY);

    tmpl
      .register_template_string("prompt", PROMPT_TEMPLATE)
      .unwrap();

    if let serde_json::Value::Object(obj) = &self.config {
      obj.iter().for_each(|(k, v)| {
        map.insert(k.clone(), v.as_str().unwrap_or_default());
      });
    }

    tmpl.render("prompt", &map).unwrap()
  }
}

pub struct Storage {
  conn: Connection,
}

impl Storage {
  pub fn new(p: &Path) -> SqlResult<Self> {
    let db = p.join("storage.sqlite3");
    let conn = Connection::open(db)?;
    let storage = Self { conn };

    storage.ensure()?;
    Ok(storage)
  }

  pub fn ensure_config(&self, id: u64) {
    info!("Ensuring guild {:?} exists", id);

    self
      .conn
      .execute(
        "INSERT INTO guild_config (guild_id, config) VALUES ( ?1, '{}') ON CONFLICT DO NOTHING",
        [id],
      )
      .ok();
  }

  pub fn update_config(&self, id: u64, key: &str, val: &str) -> SqlResult<()> {
    let key = format!("$.{}", key);
    self.ensure_config(id);
    self.conn.execute(
      "UPDATE guild_config SET config = json_set(COALESCE(config, '{}'), ?1, ?2) WHERE guild_id = ?3",
      params![key, val, id],
    )?;

    Ok(())
  }

  pub fn guild_config(&self, id: u64) -> SqlResult<GuildConfig> {
    self
      .conn
      .query_row(
        "SELECT id, guild_id, config FROM guild_config WHERE guild_id = ?1",
        [&id],
        |row| {
          Ok(GuildConfig {
            id: row.get(0).unwrap(),
            guild_id: row.get(1).unwrap(),
            config: row.get(2).unwrap(),
          })
        },
      )
      .or_else(|_| {
        Ok(GuildConfig {
          id: 0,
          guild_id: 0,
          config: serde_json::Value::Null,
        })
      })
  }

  fn ensure(&self) -> SqlResult<()> {
    self.conn.execute(
      "CREATE TABLE IF NOT EXISTS guild_config (
         id INTEGER PRIMARY KEY,
         guild_id INTEGER NOT NULL,
         config TEXT NOT NULL
       )",
      (),
    )?;

    self.conn.execute(
      "CREATE UNIQUE INDEX IF NOT EXISTS guild_config_on_guild_id ON guild_config (guild_id)",
      (),
    )?;

    self.ensure_config(0);

    Ok(())
  }
}
