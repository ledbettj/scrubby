use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Content {
  Text {
    text: String,
  },
  Image {
    source: ImageSource,
  },
  ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
  },
  ToolUse {
    id: String,
    name: String,
    input: HashMap<String, String>,
  },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ImageSource {
  Base64 {
    media_type: String,
    data: String,
  },
}
