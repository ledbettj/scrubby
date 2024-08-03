use std::collections::{HashMap, VecDeque};

use base64::prelude::*;
use log::{debug, error, trace};
use serenity::all::{Channel, ChannelId, ChannelType};
use serenity::builder::CreateMessage;
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use tokio::sync::mpsc;

use super::claude::{Client as Claude, Content, ImageSource, Interaction, Model, Role, Tool};
use crate::claude::{util, Response};
use crate::plugins::PluginEnv;

#[derive(Debug)]
pub enum BotEvent {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Bot {
  plugin_env: PluginEnv,
  claude: Claude,
  tools: Vec<Tool>,
  history: HashMap<ChannelId, VecDeque<Interaction>>,
}

pub enum BotResponse {
  Text(String),
  Embedded(CreateMessage),
}

impl Bot {
  fn new(plugin_dir: &str, claude_key: &str) -> Self {
    Self {
      plugin_env: PluginEnv::new(plugin_dir),
      claude: Claude::new(
        claude_key,
        include_str!("./claude/prompt.txt"),
        Model::Sonnet35,
      ),
      tools: vec![],
      history: HashMap::new(),
    }
  }

  pub async fn start(
    plugin_dir: &str,
    claude_key: &str,
    mut rx: mpsc::UnboundedReceiver<BotEvent>,
  ) -> () {
    let mut bot = Bot::new(plugin_dir, claude_key);

    match bot.plugin_env.load(false) {
      Err(e) => {
        error!("{}", e);
      }
      Ok(tools) => {
        bot.tools = tools;
      }
    }

    while let Some(event) = rx.recv().await {
      bot.dispatch_event(&event).await;
    }
  }

  async fn dispatch_event(&mut self, event: &BotEvent) -> () {
    match event {
      BotEvent::MessageEvent(msg, ctx) => {
        self.handle_message_event(msg, ctx).await;
      }
      BotEvent::ReadyEvent(ready, ctx) => {
        if let Err(e) = self.plugin_env.process_ready_event(&ready, &ctx) {
          error!("{}", e);
        }
      }
      BotEvent::TickEvent(ctx) => {
        if let Err(e) = self.plugin_env.process_tick_event(&ctx) {
          error!("{}", e);
        }
      }
    }
  }

  async fn message_is_respondable(msg: &Message, ctx: &Context) -> bool {
    // dont respond to your own messages
    if msg.is_own(&ctx) {
      return false;
    }

    // dont respond to blank messages
    if msg.content_safe(ctx).trim().is_empty() && msg.attachments.is_empty() {
      return false;
    }

    // always respond to private messages
    if msg.is_private() {
      return true;
    }

    // respond if it's a thread we're involved in.
    let channel = msg.channel(ctx.http()).await;
    if let Ok(Channel::Guild(GuildChannel {
      kind: ChannelType::PublicThread,
      member: Some(_),
      ..
    })) = channel
    {
      return true;
    }

    // respond if you're mentioned
    if let Ok(is_mentioned) = msg.mentions_me(&ctx).await {
      is_mentioned
    } else {
      false
    }
  }

  async fn process_reload_request(
    msg: &Message,
    ctx: &Context,
    plugin_env: &mut PluginEnv,
  ) -> anyhow::Result<Vec<Tool>> {
    match plugin_env.load(true) {
      Err(e) => {
        msg.react(&ctx.http(), '❌').await?;
        msg.reply(&ctx.http(), format!("```\n{}\n```", e)).await?;

        Err(e)
      }
      Ok(tools) => {
        msg.react(&ctx.http(), '✅').await?;
        Ok(tools)
      }
    }
  }

  fn dispatch_llm(
    history: &mut VecDeque<Interaction>,
    claude: &Claude,
    tools: &[Tool],
    plugin_env: &PluginEnv,
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

      let resp = claude.create_message(&history.make_contiguous(), &tools);
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

                let tool_content = match plugin_env.invoke_tool(&name, input) {
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

    //  see if there's a JSON blob in the middle of the text (dumb bot)
    let blob = output.join("");
    let start = blob.find("{");
    let end = blob.rfind("}");

    if let (Some(start), Some(end)) = (start, end) {
      if start + 3 < end {
        let (rest, suffix) = blob.split_at(end + 1);
        let (prefix, span) = rest.split_at(start);
        let span = util::fixup_json(span);

        if let Ok(json) = serde_json::from_str(&span) {
          if let Ok(builder) = plugin_env.build_message_json(json) {
            let mut items = vec![];
            if !prefix.trim().is_empty() {
              items.push(BotResponse::Text(prefix.to_owned()));
            }
            items.push(BotResponse::Embedded(builder));
            if !suffix.trim().is_empty() {
              items.push(BotResponse::Text(suffix.to_owned()));
            }
            return Ok(items);
          }
        }
      }
    }

    // otherwise try parsing each one individually
    Ok(
      output
        .into_iter()
        .map(|text| {
          if let Ok(json) = serde_json::from_str(&text) {
            if let Ok(builder) = plugin_env.build_message_json(json) {
              BotResponse::Embedded(builder)
            } else {
              BotResponse::Text(text)
            }
          } else {
            BotResponse::Text(text)
          }
        })
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

  async fn msg_to_content(msg: &Message, ctx: &Context) -> Vec<Content> {
    let mut items = vec![];
    let text = msg
      .content_safe(&ctx)
      .replace("@Scrubby#2153", "Scrubby")
      .trim()
      .to_owned();

    let author = msg
      .author_nick(&ctx.http)
      .await
      .unwrap_or_else(|| msg.author.name.clone());

    if !text.is_empty() {
      let text = format!("{}: {}", author, text).to_owned();
      items.push(Content::Text { text })
    }

    for attachment in &msg.attachments {
      let content_type = attachment.content_type.as_ref().map(|s| s.as_str());

      match content_type {
        Some("image/jpeg") | Some("image/png") | Some("image/gif") | Some("image/webp") => {
          if let Ok(bytes) = attachment.download().await {
            let res = util::resize_image(bytes, 600, 600);
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

  async fn handle_message_event(&mut self, msg: &Message, ctx: &Context) {
    let respondable = Self::message_is_respondable(&msg, &ctx).await;
    let msg_content = Bot::msg_to_content(&msg, &ctx).await;

    if msg_content.is_empty() {
      return;
    }

    let mut history = self.history.entry(msg.channel_id).or_default();
    Bot::ensure_valid_history(&mut history);

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

    if !respondable {
      return;
    }

    if msg.content.contains("reload") {
      if let Ok(tools) = Self::process_reload_request(&msg, &ctx, &mut self.plugin_env).await {
        self.tools = tools;
      }
      return;
    }

    if let Ok(c) = msg.channel(&ctx).await {
      match c {
        Channel::Guild(ch) => {
          let _ = ch.broadcast_typing(&ctx.http).await;
        }
        Channel::Private(ch) => {
          let _ = ch.broadcast_typing(&ctx.http).await;
        }
        _ => {}
      };
    }

    let replies = match Bot::dispatch_llm(&mut history, &self.claude, &self.tools, &self.plugin_env)
    {
      Ok(replies) => replies,
      Err(e) => {
        error!("{}", e);

        history.iter().for_each(|item| trace!("{:?}", item));

        vec![BotResponse::Text(format!("```\n{}\n```", e).to_owned())]
      }
    };

    while history.len() > 10 {
      history.drain(..2);
    }

    for r in replies.into_iter() {
      match r {
        BotResponse::Text(s) if s.trim().is_empty() => {}
        BotResponse::Text(s) => {
          msg
            .reply(&ctx.http(), s)
            .await
            .map_err(|err| error!("Failed to reply: {}", err))
            .ok();
        }
        BotResponse::Embedded(m) => {
          msg
            .channel_id
            .send_message(ctx.http(), m)
            .await
            .map_err(|err| error!("Failed to send message: {}", err))
            .ok();
        }
      }
    }
  }
}
