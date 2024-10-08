use crate::claude::{Client, Content, ImageSource, Interaction, Response, Role};
use crate::dispatcher::BotEvent;
use crate::plugins::Host;
use crate::{DEFAULT_PROMPT, PROMPT_FILE};
use base64::prelude::*;
use log::{debug, error, info, trace};
use regex::Regex;
use serenity::all::{Channel, ChannelId, ChannelType, GuildChannel};
use serenity::prelude::CacheHttp;
use std::collections::{HashMap, VecDeque};
use std::fs;
use tokio::join;
use tokio::sync::mpsc::UnboundedReceiver;

pub enum BotResponse {
  Text(String),
  Error(anyhow::Error),
}

impl Into<String> for BotResponse {
  fn into(self) -> String {
    match self {
      BotResponse::Text(s) => s.trim().into(),
      BotResponse::Error(e) => format!(":skull: Aw shit.\n```\n{}\n```", e.to_string()).into(),
    }
  }
}

pub struct EventHandler {
  host: Host,
  claude: Client,
  history: HashMap<ChannelId, VecDeque<Interaction>>,
}

impl EventHandler {
  fn new(plugin_dir: &str, claude_key: &str, prompt: &str) -> Self {
    Self {
      host: Host::new(plugin_dir),
      claude: Client::new(claude_key, prompt, crate::claude::Model::Sonnet35),
      history: HashMap::new(),
    }
  }
  pub async fn start(
    plugin_dir: &str,
    claude_key: &str,
    prompt: &str,
    mut rx: UnboundedReceiver<BotEvent>,
  ) {
    let mut handler = Self::new(plugin_dir, claude_key, prompt);
    if let Err(e) = handler.host.load() {
      error!("Loading plugins: {}", e);
    }

    while let Some(event) = rx.recv().await {
      handler.on_event(&event).await;
    }
  }

  async fn on_command(&mut self, event: &BotEvent) -> Option<Option<String>> {
    let content = &event.msg.content;
    let r = Regex::new("(?ms)set-prompt (.*)").expect("Failed to compile regex");

    if let Some(cap) = r.captures(content) {
      let prompt = cap.get(1).unwrap().as_str();
      info!("Resetting prompt: {}", prompt);
      self.claude.set_prompt(prompt);
      std::fs::write(PROMPT_FILE, prompt).expect("Failed to write prompt to storage");
      return Some(None);
    }

    if content.contains("clear-prompt") {
      self.claude.set_prompt(DEFAULT_PROMPT);
      std::fs::remove_file(PROMPT_FILE).ok();
      return Some(None);
    }

    let r = Regex::new(r#"(?ms)eval-script\s*```(.+)```"#).unwrap();
    if let Some(cap) = r.captures(content) {
      let resp = self.host.eval(cap.get(1).unwrap().as_str());
      let resp = format!("```\n{}\n```", resp).to_string();
      return Some(Some(resp));
    }

    let r = Regex::new(r#"install-script\s*(.+)"#).unwrap();
    if let Some(cap) = r.captures(content) {
      let url = cap.get(1).unwrap().as_str();
      let url = reqwest::Url::parse(url).unwrap();
      let filename = url.path_segments().unwrap().last().unwrap().to_owned();

      if let Ok(resp) = reqwest::get(url).await {
        let text = resp.text().await.unwrap();
        fs::write(format!("./storage/{}", filename), text).ok();
      }

      self.host.load().ok();
      return Some(None);
    }

    None
  }

  async fn on_event(&mut self, event: &BotEvent) {
    let (is_respondable, msg_content) = join!(
      Self::event_is_respondable(event),
      Self::msg_to_content(event)
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

    let mut history = self.history.entry(event.msg.channel_id).or_default();
    Self::ensure_valid_history(&mut history);

    match history.back_mut() {
      None
      | Some(Interaction {
        role: Role::Assistant,
        ..
      }) => {
        history.push_back(Interaction {
          role: Role::User,
          content: msg_content,
        });
      }
      Some(Interaction {
        role: Role::User,
        ref mut content,
      }) => {
        content.extend(msg_content);
      }
    }

    if !is_respondable {
      return;
    }

    if let Ok(c) = event.msg.channel(&event.ctx).await {
      match c {
        Channel::Guild(ch) => {
          let _ = ch.broadcast_typing(&event.ctx.http).await;
        }
        Channel::Private(ch) => {
          let _ = ch.broadcast_typing(&event.ctx.http).await;
        }
        _ => {}
      };
    }

    let replies = match Self::dispatch_llm(&mut history, &self.claude, &self.host).await {
      Ok(replies) => replies,
      Err(e) => {
        error!("{}", e);

        history.iter().for_each(|item| trace!("{:?}", item));

        vec![BotResponse::Error(e)]
      }
    };

    while history.len() > 10 {
      history.drain(..2);
    }

    for r in replies.into_iter() {
      let s: String = r.into();
      if !s.is_empty() {
        event
          .msg
          .reply(&event.ctx.http(), s)
          .await
          .map_err(|err| error!("Failed to reply: {}", err))
          .ok();
      }
    }
  }

  async fn event_is_respondable(event: &BotEvent) -> bool {
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
    if let Ok(Channel::Guild(GuildChannel {
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

  async fn msg_to_content(event: &BotEvent) -> Vec<Content> {
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
      items.push(Content::Text { text })
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
        Some(_) | None => {}
      }
    }

    items
  }

  async fn dispatch_llm(
    history: &mut VecDeque<Interaction>,
    claude: &Client,
    host: &Host,
  ) -> anyhow::Result<Vec<BotResponse>> {
    let mut output = vec![];

    let mut done = false;

    let last = history.back_mut().expect("No interactions to consider!?");
    let len = last.content.len();
    if len > 10 {
      last.content = last.content[(len - 10)..].to_vec();
    }

    while !done {
      done = true;
      debug!(
        "Claude Considering: {:?} with {} prior messages",
        history.back().unwrap(),
        history.len() - 1
      );

      let resp = claude
        .create_message(&history.make_contiguous(), host)
        .await;
      debug!("Claude Returned: {:?}", resp);

      match resp {
        Ok(Response::Message { content, .. }) => {
          history.push_back(Interaction {
            role: Role::Assistant,
            content: content.clone(),
          });

          let mut tool_output = vec![];

          for content in content.into_iter() {
            match content {
              Content::Text { text } => {
                output.push(text.clone());
              }
              Content::ToolUse { id, name, input } => {
                done = false;

                let tool_content = match host.invoke_tool(&name, input) {
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
              // the LLM should never respond with an image or tool result.
              Content::Image { .. } | Content::ToolResult { .. } => unreachable!(),
            }
          }
          if !tool_output.is_empty() {
            history.push_back(Interaction {
              role: Role::User,
              content: tool_output,
            });
          }
        }
        Ok(Response::Error { .. }) => unreachable!(),
        Err(e) => {
          // remove the offending user message
          history.pop_back();
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

  fn ensure_valid_history(history: &mut VecDeque<Interaction>) {
    loop {
      match history.front() {
        None => break,
        Some(Interaction {
          role: Role::Assistant,
          ..
        }) => {
          history.pop_front();
        }
        Some(Interaction {
          role: Role::User,
          content,
        }) => match content.first() {
          None | Some(Content::ToolResult { .. }) => {
            history.pop_front();
          }
          _ => break,
        },
      };
    }
  }
}