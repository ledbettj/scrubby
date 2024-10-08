use reqwest_middleware::Result;
use reqwest_retry::{default_on_request_failure, Retryable, RetryableStrategy};

pub struct Retry529 {}

impl RetryableStrategy for Retry529 {
  fn handle(&self, res: &Result<reqwest::Response>) -> Option<Retryable> {
    match res {
      Ok(resp) if resp.status() == 529 => Some(Retryable::Transient),
      Ok(_) => None,
      Err(err) => default_on_request_failure(err),
    }
  }
}
