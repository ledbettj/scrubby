use std::collections::VecDeque;

use log::debug;
use serenity::all::ChannelId;

use crate::claude::{Content, Interaction, Role};

pub struct Channel {
  id: ChannelId,
  history: VecDeque<Interaction>,
  prompt: Option<String>,
  limit: Option<usize>,
}

impl Channel {
  pub fn new(id: ChannelId, limit: Option<usize>) -> Self {
    debug!(
      "Creating new channel {:?} with interaction limit {:?}",
      id, limit
    );

    Self {
      id,
      history: VecDeque::new(),
      prompt: None,
      limit,
    }
  }

  pub fn unlimited(&mut self) {
    self.limit = None;
  }

  pub fn limited(&mut self, size: usize) {
    self.limit = Some(size);
  }

  pub fn get_history(&mut self) -> &[Interaction] {
    self.history.make_contiguous()
  }

  pub fn append_bot(&mut self, interaction: Interaction) {
    self.history.push_back(interaction);
  }

  pub fn undo_last(&mut self) {
    self.history.pop_back();
  }

  pub fn append_user(&mut self, new_content: Vec<Content>) {
    match self.history.back_mut() {
      None
      | Some(Interaction {
        role: Role::Assistant,
        ..
      }) => {
        self.history.push_back(Interaction {
          role: Role::User,
          content: new_content,
        });
      }
      Some(Interaction {
        role: Role::User,
        ref mut content,
      }) => {
        content.extend(new_content);
      }
    }
  }

  pub fn shrink(&mut self) {
    if let Some(limit) = self.limit {
      while self.history.len() > limit {
        self.history.drain(..2);
      }
    }
  }

  pub fn ensure_valid_history(&mut self) {
    loop {
      match self.history.front() {
        None => break,
        Some(Interaction {
          role: Role::Assistant,
          ..
        }) => {
          self.history.pop_front();
        }
        Some(Interaction {
          role: Role::User,
          content,
        }) => match content.first() {
          None | Some(Content::ToolResult { .. }) => {
            self.history.pop_front();
          }
          _ => break,
        },
      };
    }
  }
}
