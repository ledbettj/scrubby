use reqwest_middleware::Result;
use reqwest_retry::{default_on_request_failure, Retryable, RetryableStrategy};

pub struct Retry5xx {}

impl RetryableStrategy for Retry5xx {
  fn handle(&self, res: &Result<reqwest::Response>) -> Option<Retryable> {
    let retry_range = 500..=599;
    match res {
      Ok(resp) if retry_range.contains(&(resp.status().as_u16())) => Some(Retryable::Transient),
      Ok(_) => None,
      Err(err) => default_on_request_failure(err),
    }
  }
}
