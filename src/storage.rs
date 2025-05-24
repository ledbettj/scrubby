use handlebars::Handlebars;
use log::info;
use rusqlite::{params, Connection, Result as SqlResult};
use serde_json;
use std::collections::HashMap;
use std::path::Path;

use crate::PROMPT_TEMPLATE;

/// Default personality used when guilds don't have custom configuration.
const DEFAULT_PERSONALITY: &'static str = "Neutral and informative. Feel free to use some good-natured insults or jabs. You can use some emoji sparingly";

/// Manages guild configuration persistence using SQLite storage.
/// Handles bot personality settings and other per-guild customizations.
pub struct Storage {
  conn: Connection,
}

/// Represents a Discord guild's configuration stored in the database.
/// Contains customizable settings like personality that affect bot behavior.
#[allow(dead_code)]
#[derive(Debug)]
pub struct GuildConfig {
  id: u64,
  guild_id: u64,
  config: serde_json::Value,
}

impl GuildConfig {
  /// Generates the system prompt for Claude using guild-specific configuration.
  /// Applies custom personality settings or falls back to defaults, then renders
  /// the prompt template with the appropriate variables.
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

impl Storage {
  /// Creates a new storage instance and initializes the SQLite database.
  /// Sets up the database schema and ensures required tables exist.
  pub fn new(p: &Path) -> SqlResult<Self> {
    let db = p.join("storage.sqlite3");
    let conn = Connection::open(db)?;
    let storage = Self { conn };

    storage.ensure()?;
    Ok(storage)
  }

  /// Ensures a guild has a configuration record in the database.
  /// Creates a default configuration entry if one doesn't exist, preventing
  /// errors when the bot tries to access guild settings.
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

  /// Updates a specific configuration value for a guild.
  /// Modifies the JSON configuration by setting a key-value pair, with input
  /// validation ensured by the calling command regex patterns.
  pub fn update_config(&self, id: u64, key: &str, val: &str) -> SqlResult<()> {
    // this would be dangerous, but the key is restricted to alphanumeric characters by the cmd_regex.
    let key = format!("$.{}", key);
    self.ensure_config(id);
    self.conn.execute(
      "UPDATE guild_config SET config = json_set(COALESCE(config, '{}'), ?1, ?2) WHERE guild_id = ?3",
      params![key, val, id],
    )?;

    Ok(())
  }

  /// Retrieves a specific configuration variable for a guild.
  /// Returns the string value if found, or None if the key doesn't exist.
  pub fn get_var(&self, id: u64, key: &str) -> SqlResult<Option<String>> {
    Ok(
      self
        .guild_config(id)?
        .config
        .get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_owned()),
    )
  }

  /// Fetches the complete configuration for a guild.
  /// Returns a default configuration with null values if no record exists,
  /// ensuring the bot can always operate even for new guilds.
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

  /// Initializes the database schema and creates required tables.
  /// Sets up the guild_config table with proper indexing and creates
  /// a default global configuration (guild_id = 0) for fallback behavior.
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
