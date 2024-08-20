use super::Content;
use super::Schema;

use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
  User,
  Assistant,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum Model {
  #[serde(rename = "claude-3-haiku-20240307")]
  Haiku,
  #[serde(rename = "claude-3-sonnet-20240229")]
  Sonnet,
  #[serde(rename = "claude-3-5-sonnet-20240620")]
  Sonnet35,
  #[serde(rename = "claude-3-opus-20240229")]
  Opus,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interaction {
  pub role: Role,
  pub content: Vec<Content>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tool {
  pub name: String,
  pub description: String,
  pub input_schema: Schema,
}

#[derive(Serialize)]
struct Request<'a> {
  model: Model,
  max_tokens: usize,
  system: String,
  messages: Vec<Interaction>,
  tools: &'a [Tool],
}

#[derive(Deserialize, Debug)]
pub struct Usage {
  pub input_tokens: usize,
  pub output_tokens: usize,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum APIError {
  InvalidRequestError { message: String },
  AuthenticationError { message: String },
  PermissionError { message: String },
  NotFoundError { message: String },
  RateLimitError { message: String },
  ApiError { message: String },
  OverloadedError { message: String },
}

impl Display for APIError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidRequestError { message } => write!(f, "{}", message),
      Self::AuthenticationError { message } => write!(f, "{}", message),
      Self::PermissionError { message } => write!(f, "{}", message),
      Self::NotFoundError { message } => write!(f, "{}", message),
      Self::RateLimitError { message } => write!(f, "{}", message),
      Self::ApiError { message } => write!(f, "{}", message),
      Self::OverloadedError { message } => write!(f, "{}", message),
    }
  }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Response {
  Message {
    id: String,
    model: String,
    role: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Usage,
    content: Vec<Content>,
  },
  Error {
    error: APIError,
  },
}

impl std::error::Error for APIError {}

const API_URL: &'static str = "https://api.anthropic.com/v1/messages";

pub struct Client {
  api_key: String,
  prompt: String,
  model: Model,
}

impl Client {
  pub fn new<S: Into<String>>(api_key: S, prompt: S, model: Model) -> Self {
    Self {
      api_key: api_key.into(),
      prompt: prompt.into(),
      model,
    }
  }

  pub fn create_message(
    &self,
    messages: &[Interaction],
    tools: &[Tool],
  ) -> Result<Response, super::Error> {
    let payload = Request {
      model: self.model,
      max_tokens: 1024,
      system: self.prompt.clone(),
      messages: messages.into(),
      tools: tools.into(),
    };

    let body = serde_json::to_string(&payload)?;
    let resp = ureq::post(API_URL)
      .set("Content-Type", "application/json")
      .set("X-API-Key", &self.api_key)
      .set("Anthropic-Version", "2023-06-01")
      .send_string(&body);

    let resp = match resp {
      Ok(resp) => resp,
      Err(ureq::Error::Status(code, resp)) if code >= 400 && code < 500 => resp,
      Err(e) => return Err(super::Error::HttpError(e)),
    };

    match serde_json::from_str(&resp.into_string()?)? {
      Response::Error { error } => Err(error.into()),
      resp => Ok(resp),
    }
  }
}
