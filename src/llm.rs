use serde::{Deserialize, Serialize};
use serenity::model::channel::Message;
use std::collections::{HashMap, VecDeque};
use ureq;

#[derive(Debug)]
pub enum Error {
  IoError(std::io::Error),
  HttpError(ureq::Error),
  JsonError(serde_json::Error),
}

impl From<ureq::Error> for Error {
  fn from(value: ureq::Error) -> Self {
    Self::HttpError(value)
  }
}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Self::IoError(value)
  }
}

impl From<serde_json::Error> for Error {
  fn from(value: serde_json::Error) -> Self {
    Self::JsonError(value)
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self)
  }
}

impl std::error::Error for Error {}

const CLAUDE_URL: &'static str = "https://api.anthropic.com/v1/messages";

pub struct LLM {
  pub history: HashMap<String, VecDeque<Interaction>>,
  pub api_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Schema {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Content {
  Text {
    text: String,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct Interaction {
  role: &'static str,
  content: Vec<Content>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Tool {
  name: String,
  description: String,
  input_schema: Schema,
}

#[derive(Serialize)]
struct Request {
  model: &'static str,
  max_tokens: usize,
  system: String,
  messages: Vec<Interaction>,
  tools: Vec<Tool>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeUsage {
  input_tokens: usize,
  output_tokens: usize,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeResponse {
  id: String,
  model: String,
  role: String,
  stop_reason: Option<String>,
  stop_sequence: Option<String>,
  usage: ClaudeUsage,
  content: Vec<Content>,
}

impl LLM {
  fn request(&self, messages: &VecDeque<Interaction>) -> Result<ureq::Response, ureq::Error> {
    let payload = Request {
      model: "claude-3-haiku-20240307",
      max_tokens: 1024,
      system: "".into(),
      messages: messages.clone().into(),
      tools: vec![Tool {
        name: "weather".into(),
        description: "Return the weather forecast for a given location".into(),
        input_schema: Schema::Object {
          properties: [(
            "location".to_owned(),
            Schema::String {
              description:
                "the city and state to get the forecast for, for example San Francisco, CA".into(),
            },
          )]
          .into_iter()
          .collect(),
          required: vec!["location".into()],
        },
      }],
    };
    let body = serde_json::to_string(&payload).unwrap();
    println!("{:?}", body);
    ureq::post(CLAUDE_URL)
      .set("Content-Type", "application/json")
      .set("X-API-Key", &self.api_key)
      .set("Anthropic-Version", "2023-06-01")
      .set("Anthropic-Beta", "tools-2024-05-16")
      .send_string(&body)
  }

  pub fn respond(
    &mut self, // msg: &Message
    author: String,
    content: String,
  ) -> Result<String, Error> {
    let author = author.clone();
    let mut interaction = Interaction {
      role: "user",
      content: vec![Content::Text {
        text: content.clone(),
      }],
    };

    let mut output: Vec<String> = vec![];
    let mut done = false;

    while !done {
      self
        .history
        .entry(author.clone())
        .or_default()
        .push_back(interaction.clone());

      let messages = self.history.get(&author).unwrap();
      let resp = match self.request(&messages) {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) if code >= 400 && code < 500 => resp,
        Err(e) => return Err(Error::HttpError(e)),
      };
      let s = resp.into_string()?;
      let c: ClaudeResponse = serde_json::from_str(&s)?;
      done = true;

      self
        .history
        .get_mut(&author)
        .unwrap()
        .push_back(Interaction {
          role: "assistant",
          content: c.content.clone(),
        });

      for content in c.content.iter() {
        match content {
          Content::Text { text } => {
            output.push(text.clone());
          }
          Content::ToolResult { .. } => unreachable!(),
          Content::ToolUse { id, .. } => {
            done = false;
            interaction = Interaction {
              role: "user",
              content: vec![Content::ToolResult {
                tool_use_id: id.clone(),
                is_error: false,
                content: "The weather will be sunny and 80 degrees.".into(),
              }],
            }
          }
        }
      }
    }

    Ok(output.join("\n"))
  }
}
