use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Citation {
  pub r#type: String,
  pub url: String,
  pub title: String,
  pub encrypted_index: String,
  pub cited_text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Content {
  Text {
    text: String,
    citations: Option<Vec<Citation>>,
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
    input: serde_json::Value,
  },
  ServerToolUse {
    id: String,
    name: String,
    input: serde_json::Value,
  },
  WebSearchToolResult {
    tool_use_id: String,
    content: serde_json::Value,
  },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ImageSource {
  Base64 { media_type: String, data: String },
}

impl Content {
  pub fn is_image(&self) -> bool {
    match self {
      Self::Image { .. } => true,
      _ => false,
    }
  }

  pub fn text<S: Into<String>>(text: S) -> Self {
    Self::Text {
      text: text.into(),
      citations: None,
    }
  }
}
