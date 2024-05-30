use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io;
use ureq;
#[derive(Debug)]
pub enum Error {
  IoError(io::Error),
  HttpError(ureq::Error),
  JsonError(serde_json::Error),
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

impl Display for Error {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}", self)
  }
}

impl std::error::Error for Error {}
