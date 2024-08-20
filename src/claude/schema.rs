use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Schema {
  Object {
    properties: HashMap<String, Schema>,
    required: Vec<String>,
  },
  String {
    description: String,
  },
  Number {
    description: String,
  },
  #[serde(rename_all = "camelCase")]
  Array {
    description: String,
    items: Box<Schema>,
  }
}

impl Default for Schema {
  fn default() -> Self {
    Schema::Object {
      properties: HashMap::default(),
      required: Vec::default(),
    }
  }
}
