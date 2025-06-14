use super::retry::Retry5xx;
use super::Content;
use super::Schema;

use reqwest_middleware::ClientBuilder;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
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
  #[serde(rename = "claude-3-5-sonnet-latest")]
  Sonnet35,
  #[serde(rename = "claude-3-7-sonnet-latest")]
  Sonnet37,
  #[serde(rename = "claude-sonnet-4-20250514")]
  Sonnet4,
  #[serde(rename = "claude-3-5-haiku-latest")]
  Haiku35,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interaction {
  pub role: Role,
  pub content: Vec<Content>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case", untagged)]
pub enum Tool {
  WebSearch {
    r#type: String,
    name: String,
    max_uses: usize,
    blocked_domains: Option<Vec<String>>,
  },
  CodeExecution {
    r#type: String,
    name: String,
  },
  Custom {
    name: String,
    description: String,
    input_schema: Schema,
  },
}

impl Tool {
  pub fn name(&self) -> &str {
    match self {
      Tool::WebSearch { name, .. } => name,
      Tool::Custom { name, .. } => name,
      Tool::CodeExecution { name, .. } => name,
    }
  }

  pub fn web_search(max_uses: usize, blocked_domains: Option<Vec<String>>) -> Self {
    Tool::WebSearch {
      r#type: "web_search_20250305".into(),
      name: "web_search".into(),
      max_uses,
      blocked_domains,
    }
  }

  pub fn code_execution() -> Self {
    Tool::CodeExecution {
      r#type: "code_execution_20250522".into(),
      name: "code_execution".into(),
    }
  }
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
pub struct ModelDetails {
  pub id: String,
  pub display_name: String,
  pub r#type: String,
  pub created_at: String,
}

#[derive(Deserialize, Debug)]
pub struct ListModelsResponse {
  pub data: Vec<ModelDetails>,
  pub first_id: String,
  pub has_more: bool,
  pub last_id: String,
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
  model: Model,
}

impl Client {
  pub fn new<S: Into<String>>(api_key: S, model: Model) -> Self {
    Self {
      api_key: api_key.into(),
      model,
    }
  }

  // this function is only for testing and may panic
  pub async fn models(&self) -> ListModelsResponse {
    let client = reqwest::Client::new();
    let resp = client
      .get("https://api.anthropic.com/v1/models")
      .header("X-API-Key", &self.api_key)
      .header("Anthropic-Version", "2023-06-01")
      .send()
      .await
      .unwrap();

    if !resp.status().is_success() {
      panic!();
    }

    let body = resp
      .text()
      .await
      .map_err(|e| reqwest_middleware::Error::Reqwest(e))
      .unwrap();

    let resp: ListModelsResponse = serde_json::from_str(&body).unwrap();
    resp
  }

  pub async fn create_message(
    &self,
    model_override: Option<Model>,
    messages: &[Interaction],
    tools: &[Tool],
    prompt: String,
  ) -> Result<Response, super::Error> {
    let payload = Request {
      model: model_override.unwrap_or(self.model),
      max_tokens: 1024,
      system: prompt,
      messages: messages.into(),
      tools,
    };

    let body = serde_json::to_string(&payload)?;
    let retry_policy = ExponentialBackoff::builder()
      .base(2)
      .build_with_max_retries(3);

    let client = ClientBuilder::new(reqwest::Client::new())
      .with(RetryTransientMiddleware::new_with_policy_and_strategy(
        retry_policy,
        Retry5xx {},
      ))
      .build();

    let resp = client
      .post(API_URL)
      .header("Content-Type", "application/json")
      .header("X-API-Key", &self.api_key)
      .header("Anthropic-Version", "2023-06-01")
      .header(
        "Anthropic-Beta",
        "code-execution-2025-05-22,tools-2024-05-16",
      )
      .body(body)
      .send()
      .await?;

    if let Err(err) = resp.error_for_status_ref() {
      if !resp.status().is_client_error() {
        return Err(super::Error::HttpError(err.into()));
      }
    }

    let body = resp
      .text()
      .await
      .map_err(|e| reqwest_middleware::Error::Reqwest(e))?;

    match serde_json::from_str(&body)? {
      Response::Error { error } => Err(error.into()),
      resp => Ok(resp),
    }
  }
}
