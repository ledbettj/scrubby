use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io;

#[derive(Debug)]
pub enum Error {
  IoError(io::Error),
  HttpError(reqwest::Error),
  JsonError(serde_json::Error),
  APIError(super::api::APIError),
}

impl From<reqwest::Error> for Error {
  fn from(value: reqwest::Error) -> Self {
    Self::HttpError(value)
  }
}

impl From<io::Error> for Error {
  fn from(value: io::Error) -> Self {
    Self::IoError(value)
  }
}

impl From<serde_json::Error> for Error {
  fn from(value: serde_json::Error) -> Self {
    Self::JsonError(value)
  }
}

impl From<super::api::APIError> for Error {
  fn from(value: super::api::APIError) -> Self {
    Self::APIError(value)
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      Self::IoError(e) => write!(f, "IO Error: {}", e),
      Self::HttpError(e) => write!(f, "HTTP Error: {}", e),
      Self::JsonError(e) => write!(f, "JSON Error: {}", e),
      Self::APIError(e) => write!(f, "API Error: {}", e),
    }
  }
}

impl std::error::Error for Error {}
