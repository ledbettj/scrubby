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

/// the default Schema is an empty Schema::Object
impl Default for Schema {
  fn default() -> Self {
    Schema::Object {
      properties: HashMap::default(),
      required: Vec::default(),
    }
  }
}

impl Schema {
  /// Create a new Schema::String
  pub fn string<S: Into<String>>(description: S) -> Self {
    Schema::String {
      description: description.into(),
    }
  }

  /// Create a new Schema::Integer
  pub fn integer<S: Into<String>>(description: S) -> Self {
    Schema::Integer {
      description: description.into(),
    }
  }

  /// Create a new Schema::Object
  pub fn object() -> Self {
    Self::default()
  }

  /// Add a new property to the Schema::Object
  pub fn with_property<S: Into<String>>(
    mut self,
    name: S,
    schema: Schema,
    is_required: bool,
  ) -> Self {
    let name = name.into();
    if let Schema::Object {
      properties,
      required,
      ..
    } = &mut self
    {
      if is_required {
        required.push(name.clone());
      }
      properties.insert(name, schema);
    }
    self
  }
}
