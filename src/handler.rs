use crate::audio::AudioHandler;
use crate::channel::Channel;
use crate::claude::{
  self, Client, Content, ImageSource, Interaction, Model, Response, Role, Tool, tools::*,
};
use crate::dispatcher::{BotEvent, MsgEvent, ReadyEvent, ThreadUpdateEvent};
use crate::storage::Storage;
use base64::prelude::*;
use itertools::Itertools;
use log::{debug, error, info, trace};
use regex::{Captures, Regex};
use serenity::all::{
  Channel as DChannel, ChannelId, ChannelType, CreateAttachment, CreateMessage, GuildChannel,
};
use serenity::prelude::CacheHttp;
use std::collections::HashMap;
use std::path::Path;
use tokio::join;
use tokio::sync::mpsc::UnboundedReceiver;

/// Represents the bot's response to a user message.
/// Can be either successful text output or an error that occurred during processing.
pub enum BotResponse {
  Text(String),
  Error(anyhow::Error),
}

/// Converts bot responses into Discord-formatted strings.
/// Formats errors with skull emoji and code blocks for visibility.
impl From<BotResponse> for String {
  fn from(val: BotResponse) -> Self {
    match val {
      BotResponse::Text(s) => s.trim().into(),
      BotResponse::Error(e) => format!(":skull: \n```\n{}\n```", e.to_string()).into(),
    }
  }
}

/// Main bot event handler that processes Discord events and generates AI responses.
/// Manages conversation state, bot commands, and Claude AI integration.
pub struct EventHandler<'a> {
  claude: Client,
  channels: HashMap<ChannelId, Channel>,
  storage: Storage,
  commands: Vec<Command>,
  tools: ToolCollection,
  audio: Option<crate::audio::AudioHandler<'a>>,
}

/// Represents a bot command that can be invoked via regex pattern matching.
/// Commands provide direct bot functionality like configuration management.
struct Command {
  regex: Regex,
  invoke: fn(&mut EventHandler, &Captures<'_>, &MsgEvent) -> Option<String>,
}

impl<'a> EventHandler<'a> {
  /// Creates a new event handler with Claude client and bot commands.
  /// Initializes storage, sets up built-in commands, and configures audio if enabled.
  fn new(storage_dir: &str, claude_key: &str) -> Self {
    let set = Command {
      regex: Regex::new(r#"(?ms)set-var\s+([A-Za-z_]+)\s*=\s*(.+)"#).unwrap(),
      invoke: |handler, cap, event| {
        let key = cap.get(1).unwrap().as_str().to_lowercase();
        let val = cap.get(2).unwrap().as_str().trim();
        if let Some(id) = event.msg.guild_id {
          info!("Setting {:?} {} = {}", id, key, val);
          handler.storage.update_config(id.into(), &key, val).ok();
        }
        None
      },
    };

    let get = Command {
      regex: Regex::new(r#"(?ms)get-var\s+([A-Za-z_]+)"#).unwrap(),
      invoke: |handler, cap, event| {
        let key = cap.get(1).unwrap().as_str().to_lowercase();
        if let Some(id) = event.msg.guild_id {
          info!("Getting {:?} {}", id, key);
          if let Ok(Some(val)) = handler.storage.get_var(id.into(), &key) {
            return Some(val);
          }
        }
        None
      },
    };

    let forget = Command {
      regex: Regex::new(r#"(?ms)forget-history"#).unwrap(),
      invoke: |handler, _cap, event| {
        let id = event.msg.channel_id;
        info!("Forgetting history for {:?}", id);
        handler.channels.remove(&id);
        return Some("I didn't see nothin'".into());
      },
    };

    let audio = if std::env::var("AUDIO_ENABLED").is_ok() {
      Some(crate::audio::AudioHandler::new("./storage/base.bin").unwrap())
    } else {
      None
    };

    Self {
      claude: Client::new(claude_key, claude::Model::Sonnet4),
      channels: HashMap::new(),
      storage: Storage::new(Path::new(storage_dir)).unwrap(),
      commands: vec![set, get, forget],
      tools: vec![],
      audio,
    }
  }

  /// Main event processing loop that handles all incoming Discord events.
  /// Runs continuously, processing events from the dispatcher until shutdown.
  pub async fn start(storage_dir: &str, claude_key: &str, mut rx: UnboundedReceiver<BotEvent>) {
    let mut handler = Self::new(storage_dir, claude_key);

    while let Some(event) = rx.recv().await {
      handler.on_event(&event).await;
    }
  }

  /// Processes bot commands embedded in user messages.
  /// Returns Some(Some(response)) for commands with output, Some(None) for silent commands, None if no command matched.
  async fn on_command(&mut self, event: &MsgEvent) -> Option<Option<String>> {
    let content = &event.msg.content;

    for cmd in &self.commands {
      if let Some(cap) = cmd.regex.captures(content) {
        return Some((cmd.invoke)(self, &cap, event));
      }
    }

    None
  }

  /// Routes incoming Discord events to their appropriate handlers.
  /// Dispatches messages, ready events, and thread updates to specialized methods.
  async fn on_event(&mut self, event: &BotEvent) {
    match event {
      BotEvent::Message(m) => self.on_message(m).await,
      BotEvent::Ready(r) => self.on_ready(r),
      BotEvent::ThreadUpdate(t) => self.on_thread_update(t),
    }
  }

  /// Handles Discord thread lifecycle events for conversation cleanup.
  /// Removes conversation history when threads are archived to prevent memory leaks.
  fn on_thread_update(&mut self, event: &ThreadUpdateEvent) {
    if let Some(metadata) = event.new.thread_metadata {
      if metadata.archived {
        debug!("Cleaning up channel {:?}", event.new.id);
        self.channels.remove(&event.new.id);
      }
    }
  }

  /// Initializes bot state when Discord connection is established.
  /// Ensures database configuration exists for all guilds the bot has access to.
  fn on_ready(&self, event: &ReadyEvent) {
    event
      .guilds
      .iter()
      .for_each(|&guild_id| self.storage.ensure_config(guild_id.into()));
  }

  /// Core message processing logic that handles user interactions.
  /// Determines response eligibility, processes commands, manages conversation history,
  /// and coordinates with Claude AI to generate responses.
  async fn on_message(&mut self, event: &MsgEvent) {
    let (is_respondable, msg_content) = join!(
      Self::event_is_respondable(event),
      Self::msg_to_content(event, &self.audio)
    );

    if is_respondable {
      match self.on_command(&event).await {
        Some(None) => {
          event.msg.react(&event.ctx.http(), '✅').await.ok();
          return;
        }
        Some(Some(s)) => {
          event.msg.reply(&event.ctx.http(), s).await.ok();
          return;
        }
        None => {}
      }
    }

    let id = event.msg.channel_id;
    let limit = match event
      .msg
      .channel(event.ctx.http())
      .await
      .ok()
      .and_then(|ch| ch.guild())
      .map(|gc| gc.kind)
    {
      Some(ChannelType::PublicThread) => None,
      _ => Some(10),
    };
    let mut channel = self
      .channels
      .entry(id)
      .or_insert_with(|| Channel::new(id, limit));

    channel.ensure_valid_history();
    channel.user_message(msg_content);

    if !is_respondable {
      return;
    }

    // send a typing indicator to the channel.
    if let Ok(c) = event.msg.channel(&event.ctx).await {
      match c {
        DChannel::Guild(ch) => {
          let _ = ch.broadcast_typing(&event.ctx.http).await;
        }
        DChannel::Private(ch) => {
          let _ = ch.broadcast_typing(&event.ctx.http).await;
        }
        _ => {}
      };
    }

    // events in a channel or thread will have a GuildId. direct messages will not.
    // in that case, fall back to guild ID = 0 which is the global fallback configuration.
    let guild_id = event.msg.guild_id.map(|id| id.into()).unwrap_or(0u64);

    // we should always get a config back here, unless an SQL error occurs.
    let prompt = self
      .storage
      .guild_config(guild_id)
      .map(|cfg| cfg.system())
      .unwrap_or_else(|_| "".into());

    let replies =
      match Self::dispatch_llm(&mut channel, prompt, &mut self.tools, &self.claude).await {
        Ok(replies) => replies,
        Err(e) => {
          error!("{}", e);

          channel
            .history()
            .iter()
            .for_each(|item| trace!("{:?}", item));

          vec![BotResponse::Error(e)]
        }
      };

    channel.shrink();

    // bundle replies into single message.
    let reply = replies
      .into_iter()
      .map(|r| Into::<String>::into(r))
      .filter(|s| !s.is_empty())
      .join(" ");

    // check if the message is too long (2_000 characters).
    // if so, convert it to a file attachment.
    if reply.len() > 2_000 {
      let attachment = CreateAttachment::bytes(reply.as_bytes(), "scrubby.txt");
      event
        .msg
        .channel_id
        .send_message(
          &event.ctx.http(),
          CreateMessage::new()
            .add_file(attachment)
            .content(":eyes:")
            .reference_message(&event.msg),
        )
        .await
        .map_err(|err| error!("Failed to send message: {}", err))
        .ok();
    } else if !reply.is_empty() {
      event
        .msg
        .reply(&event.ctx.http(), reply)
        .await
        .map_err(|err| error!("Failed to reply: {}", err))
        .ok();
    }
    // each reply sent individually
    //
    // for r in replies.into_iter() {
    //   let s: String = r.into();
    //   if !s.is_empty() {
    //     event
    //       .msg
    //       .reply(&event.ctx.http(), s)
    //       .await
    //       .map_err(|err| error!("Failed to reply: {}", err))
    //       .ok();
    //   }
    // }
  }

  /// Determines if the bot should respond to a particular message.
  /// Considers factors like self-messages, mentions, DMs, and thread participation.
  async fn event_is_respondable(event: &MsgEvent) -> bool {
    // dont respond to your own messages
    if event.msg.author.id == event.ctx.cache.current_user().id {
      return false;
    }

    // dont respond to blank messages
    if event.msg.content_safe(&event.ctx).trim().is_empty() && event.msg.attachments.is_empty() {
      return false;
    }

    // always respond to private messages
    if event.msg.guild_id.is_none() {
      return true;
    }

    // respond if it's a thread we're involved in.
    let channel = event.msg.channel(event.ctx.http()).await;
    if let Ok(DChannel::Guild(GuildChannel {
      kind: ChannelType::PublicThread,
      member: Some(_),
      ..
    })) = channel
    {
      return true;
    }

    // respond if you're mentioned
    if let Ok(is_mentioned) = event.msg.mentions_me(&event.ctx).await {
      is_mentioned
    } else {
      false
    }
  }

  /// Converts Discord message data into Claude-compatible content format.
  /// Processes text, images, audio transcriptions, and document attachments for AI consumption.
  async fn msg_to_content(event: &MsgEvent, audio: &'_ Option<AudioHandler<'_>>) -> Vec<Content> {
    let mut items = vec![];
    let text = event
      .msg
      .content_safe(&event.ctx)
      .replace("@Scrubby#2153", "Scrubby")
      .trim()
      .to_owned();

    let author = event
      .msg
      .author_nick(&event.ctx.http)
      .await
      .unwrap_or_else(|| event.msg.author.name.clone());

    if !text.is_empty() {
      let text = format!("{}: {}", author, text).to_owned();
      items.push(Content::text(text));
    }

    for attachment in &event.msg.attachments {
      let content_type = attachment.content_type.as_ref().map(|s| s.as_str());

      match content_type {
        Some("image/jpeg") | Some("image/png") | Some("image/gif") | Some("image/webp") => {
          if let Ok(bytes) = attachment.download().await {
            let res = crate::claude::util::resize_image(bytes, 600, 600);
            if let Ok(bytes) = res {
              let data = BASE64_STANDARD.encode(&bytes);

              items.push(Content::Image {
                source: ImageSource::Base64 {
                  media_type: "image/png".into(),
                  data,
                },
              })
            }
          }
        }
        Some("audio/ogg") | Some("application/ogg") if audio.is_some() => {
          let audio = audio.as_ref().unwrap();
          if let Ok(bytes) = attachment.download().await {
            if let Ok(transcript) = audio.tts(&bytes) {
              debug!("Transcription output: {:?}", &transcript);
              if !transcript.is_empty() {
                items.push(Content::text(transcript));
              }
            } else {
              items.push(Content::text(
                "I shared an audio file with you, but you didn't understand it",
              ));
            }
          }
        }
        Some("text/plain") => {
          if let Ok(bytes) = attachment.download().await {
            match String::from_utf8(bytes) {
              Err(e) => error!("Failed to decode text attachment: {}", e),
              Ok(s) => items.push(Content::text(format!(
                "<document name=\"{}\">\n{}</document>",
                attachment.filename, s
              ))),
            }
          }
        }
        Some(t) if t.contains("charset=utf-8") => {
          if let Ok(bytes) = attachment.download().await {
            match String::from_utf8(bytes) {
              Err(e) => error!("Failed to decode text attachment: {}", e),
              Ok(s) => items.push(Content::text(format!(
                "<document name=\"{}\">\n{}</document>",
                attachment.filename, s
              ))),
            }
          }
        }
        Some(t) => debug!("Unhandled attachment content type: {}", t),
        None => {}
      }
    }

    items
  }

  /// Manages the conversation flow with Claude AI, including tool usage.
  /// Handles the request-response cycle, processes tool calls, and manages model selection
  /// based on conversation content (images require vision-capable models).
  async fn dispatch_llm(
    channel: &mut Channel,
    prompt: String,
    mut tools: &mut ToolCollection,
    claude: &Client,
  ) -> anyhow::Result<Vec<BotResponse>> {
    let mut output = vec![];

    let mut done = false;

    while !done {
      let model = if channel.history_has_images() {
        None
      } else {
        Some(Model::Haiku35)
      };
      let history = channel.history();
      done = true;
      debug!(
        "Claude Considering: {:?} with {} prior messages",
        history.last().unwrap(),
        history.len() - 1
      );

      let tool_meta = tools
        .iter()
        .map(|t| t.metadata())
        .cloned()
        .collect::<Vec<_>>();

      let resp = claude
        .create_message(model, &history, &tool_meta, prompt.clone())
        .await;
      debug!("Claude Returned: {:?}", resp);

      match resp {
        Ok(Response::Message { content, .. }) => {
          channel.bot_message(Interaction {
            role: Role::Assistant,
            content: content.clone(),
          });

          let mut tool_output = vec![];

          for content in content.into_iter() {
            match content {
              Content::Text {
                text,
                citations: Some(citations),
              } if !citations.is_empty() => {
                let citations = citations
                  .iter()
                  .map(|c| format!("`{}`", c.url))
                  .unique()
                  .join(" ");
                output.push(format!("{} [{}]", text, citations));
              }
              Content::Text { text, .. } => {
                output.push(text.clone());
              }
              Content::ToolUse { id, name, input } => {
                done = false;

                let tool_content = match crate::claude::tools::invoke_tool(&mut tools, &name, input)
                {
                  Err(e) => Content::ToolResult {
                    tool_use_id: id,
                    content: e.to_string(),
                    is_error: true,
                  },
                  Ok(None) => Content::ToolResult {
                    tool_use_id: id,
                    content: "<no output>".into(),
                    is_error: false,
                  },
                  Ok(Some(s)) => Content::ToolResult {
                    tool_use_id: id,
                    content: s.into(),
                    is_error: false,
                  },
                };
                tool_output.push(tool_content);
              }
              Content::ServerToolUse { .. }
              | Content::WebSearchToolResult { .. }
              | Content::CodeExecutionToolResult { .. } => { /* nothing to do here */ }
              // the LLM should never respond with an image or tool result.
              Content::Image { .. } | Content::ToolResult { .. } => unreachable!(),
            }
          }
          if !tool_output.is_empty() {
            channel.bot_message(Interaction {
              role: Role::User,
              content: tool_output,
            });
          }
        }
        Ok(Response::Error { .. }) => unreachable!(),
        Err(e) => {
          // remove the offending user message
          channel.undo_last();
          let _ = Err(e)?;
        }
      }
    }

    Ok(
      output
        .into_iter()
        .map(|text| BotResponse::Text(text))
        .collect(),
    )
  }
}
