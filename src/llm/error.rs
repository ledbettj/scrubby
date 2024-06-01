use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io;
use ureq;
#[derive(Debug)]
pub enum Error {
  IoError(io::Error),
  HttpError(ureq::Error),
  JsonError(serde_json::Error),
  ClaudeError(super::ClaudeError),
}

impl From<ureq::Error> for Error {
  fn from(value: ureq::Error) -> Self {
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

impl From<super::ClaudeError> for Error {
  fn from(value: super::ClaudeError) -> Self {
    Self::ClaudeError(value)
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      Self::IoError(e) => write!(f, "IO Error: {}", e),
      Self::HttpError(e) => write!(f, "HTTP Error: {}", e),
      Self::JsonError(e) => write!(f, "JSON Error: {}", e),
      Self::ClaudeError(e) => write!(f, "Claude Error: {}", e),
    }
  }
}

impl std::error::Error for Error {}
