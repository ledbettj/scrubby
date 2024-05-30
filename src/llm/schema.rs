use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Schema {
  Object {
    properties: HashMap<String, Schema>,
    required: Vec<String>,
  },
  String {
    description: String,
  },
  Integer {
    description: String,
  },
}

impl Default for Schema {
  fn default() -> Self {
    Schema::Object {
      properties: HashMap::default(),
      required: Vec::default(),
    }
  }
}
