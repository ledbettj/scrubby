use std::collections::VecDeque;

use log::debug;
use serenity::all::ChannelId;

use crate::claude::{Content, Interaction, Role};

/// Represents a Discord channel with conversation history.
/// Each channel maintains its own conversation context and history limits for Claude AI interactions.
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

  /// Returns the conversation history as a contiguous slice.
  /// This ensures the VecDeque is organized in memory for efficient access.
  pub fn history(&mut self) -> &[Interaction] {
    self.hist.make_contiguous()
  }

  /// Determines if the conversation history contains any image content.
  /// This affects which Claude model variant is used, as images require vision-capable models.
  pub fn history_has_images(&self) -> bool {
    self
      .hist
      .iter()
      .any(|interaction| interaction.content.iter().any(|content| content.is_image()))
  }

  /// Appends a bot (assistant) response to the conversation history.
  pub fn bot_message(&mut self, interaction: Interaction) {
    self.hist.push_back(interaction);
  }

  /// Removes the most recent interaction from history.
  /// Used for error recovery when a message processing fails.
  pub fn undo_last(&mut self) {
    self.hist.pop_back();
  }

  /// Adds user content to the conversation history.
  /// Consecutive user messages are merged into a single interaction to maintain
  /// proper conversation flow for Claude's alternating user/assistant pattern.
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

  /// Enforces the conversation history size limit by removing old messages.
  /// Messages are removed in pairs (user+assistant) to maintain conversation coherence
  /// and prevent leaving the history in an invalid state for Claude processing.
  pub fn shrink(&mut self) {
    if let Some(limit) = self.limit {
      while self.hist.len() > limit {
        self.hist.drain(..2);
      }
    }
  }

  /// Validates and cleans the conversation history for Claude API consumption.
  /// Removes leading assistant messages and empty/tool-only user messages that
  /// would cause Claude API errors due to invalid conversation structure.
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::claude::Content;

  #[test]
  fn test_user_message_merging() {
    let mut channel = Channel::new(ChannelId::new(123), None);

    channel.user_message(vec![Content::text("Hello")]);
    channel.user_message(vec![Content::text("World")]);

    assert_eq!(channel.history().len(), 1);
    assert_eq!(
      channel.history()[0].content,
      vec![Content::text("Hello"), Content::text("World")]
    );
  }
}
