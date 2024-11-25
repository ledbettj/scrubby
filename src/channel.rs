use std::collections::VecDeque;

use log::debug;
use serenity::all::ChannelId;

use crate::claude::{Content, Interaction, Role};

/// A a Discord channel.
/// Claude treats each channel individually, so each channel has its own history and limit on messages in the history window.
pub struct Channel {
  hist: VecDeque<Interaction>,
  limit: Option<usize>,
}

impl Channel {
  /// Instantiate a new channel with an optional limit on the number of messages in the history window.
  pub fn new(id: ChannelId, limit: Option<usize>) -> Self {
    debug!(
      "Creating new channel {:?} with interaction limit {:?}",
      id, limit
    );

    Self {
      hist: VecDeque::new(),
      limit,
    }
  }

  pub fn history(&mut self) -> &[Interaction] {
    self.hist.make_contiguous()
  }

  /// Check if the interaction history contains any images.
  /// If so, we'll need to use an image-enabled LLM to process the next message.
  pub fn history_has_images(&self) -> bool {
    self
      .hist
      .iter()
      .any(|interaction| interaction.content.iter().any(|content| content.is_image()))
  }

  /// Add a new assistant message to the history.
  pub fn bot_message(&mut self, interaction: Interaction) {
    self.hist.push_back(interaction);
  }

  /// Remove the last message from the history, in case something went wrong
  pub fn undo_last(&mut self) {
    self.hist.pop_back();
  }

  /// Add a new user message to the history.
  /// If the previous interaction was also a user message,
  /// the new message content will be appended to the previous one.
  /// otherwise, a new user interaction will be created.
  pub fn user_message(&mut self, new_content: Vec<Content>) {
    match self.hist.back_mut() {
      None
      | Some(Interaction {
        role: Role::Assistant,
        ..
      }) => {
        self.hist.push_back(Interaction {
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

  /// Reduce the size of the history to the limit for this channel,
  /// removing the oldest messages first and attempting to remove them in
  /// pairs to avoid leaving the history in an unprocessable state.
  pub fn shrink(&mut self) {
    if let Some(limit) = self.limit {
      while self.hist.len() > limit {
        self.hist.drain(..2);
      }
    }
  }

  /// Ensure that the history is in a valid state for processing.
  /// That means that there cannot be two back-to-back assistant messages.
  pub fn ensure_valid_history(&mut self) {
    loop {
      match self.hist.front() {
        None => break,
        Some(Interaction {
          role: Role::Assistant,
          ..
        }) => {
          self.hist.pop_front();
        }
        Some(Interaction {
          role: Role::User,
          content,
        }) => match content.first() {
          None | Some(Content::ToolResult { .. }) => {
            self.hist.pop_front();
          }
          _ => break,
        },
      };
    }
  }
}
