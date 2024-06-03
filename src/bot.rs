use std::collections::{HashMap, VecDeque};

use base64::prelude::*;
use colored::Colorize;
use serenity::all::{Channel, ChannelId, ChannelType};
use serenity::builder::CreateMessage;
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use tokio::sync::mpsc;

use super::claude::{Client as Claude, Content, ImageSource, Interaction, Model, Role, Tool};
use crate::claude::{self, Response};
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
        Model::Sonnet,
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
        println!("[{}] {}", "Error".red().bold(), e);
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
        if !Self::message_is_respondable(&msg, &ctx).await {
          return;
        }

        if msg.content.contains("reload") {
          if let Ok(tools) = Self::process_reload_request(&msg, &ctx, &mut self.plugin_env).await {
            self.tools = tools;
          }
          return;
        }
        let content = Bot::msg_to_content(&msg, &ctx).await;
        let mut history = self.history.entry(msg.channel_id).or_default();
        let replies = match Bot::dispatch_llm(
          content,
          &mut history,
          &self.claude,
          &self.tools,
          &self.plugin_env,
        ) {
          Ok(replies) => replies,
          Err(e) => vec![BotResponse::Text(format!("```\n{}\n```", e).to_owned())],
        };

        while history.len() > 10 {
          history.drain(..2);
        }

        for r in replies.into_iter() {
          match r {
            BotResponse::Text(s) => {
              msg
                .reply(&ctx.http(), s)
                .await
                .map_err(|err| println!("[{}] Failed to reply: {}", "Error".red().bold(), err))
                .ok();
            }
            BotResponse::Embedded(m) => {
              msg
                .channel_id
                .send_message(ctx.http(), m)
                .await
                .map_err(|err| {
                  println!("[{}] Failed to send message: {}", "Error".red().bold(), err)
                })
                .ok();
            }
          }
        }
      }
      BotEvent::ReadyEvent(ready, ctx) => {
        if let Err(err) = self.plugin_env.process_ready_event(&ready, &ctx) {
          println!("[{}] {}", "Error".red().bold(), err);
        }
      }
      BotEvent::TickEvent(ctx) => {
        if let Err(err) = self.plugin_env.process_tick_event(&ctx) {
          println!("[{}] {}", "Error".red().bold(), err);
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
    content: Vec<Content>,
    history: &mut VecDeque<Interaction>,
    claude: &Claude,
    tools: &[Tool],
    plugin_env: &PluginEnv,
  ) -> anyhow::Result<Vec<BotResponse>> {
    let mut output = vec![];

    if content.is_empty() {
      return Ok(vec![]);
    }

    let interaction = Interaction {
      role: Role::User,
      content,
    };

    history.push_back(interaction);
    let mut done = false;

    while !done {
      done = true;
      println!(
        "[{}] Claude Considering: {:?} with {} prior messages",
        "Debug".white().bold(),
        history.back().unwrap(),
        history.len() - 1
      );
      let resp = claude.create_message(&history.make_contiguous(), &tools);
      println!("[{}] Claude Returned: {:?}", "Debug".white().bold(), resp);
      match resp {
        Ok(Response::Message { content, .. }) => {
          history.push_back(Interaction {
            role: Role::Assistant,
            content: content.clone(),
          });

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
                history.push_back(Interaction {
                  role: Role::User,
                  content: vec![tool_content],
                });
              }
              // the LLM should never respond with an image or tool result.
              Content::Image { .. } | Content::ToolResult { .. } => unreachable!(),
            }
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

  async fn msg_to_content(msg: &Message, ctx: &Context) -> Vec<Content> {
    let mut items = vec![];
    let text = msg
      .content_safe(&ctx)
      .replace("@Scrubby#2153", "Scrubby")
      .trim()
      .to_owned();

    if !text.is_empty() {
      items.push(Content::Text { text })
    }

    for attachment in &msg.attachments {
      let content_type = attachment.content_type.as_ref().map(|s| s.as_str());

      match content_type {
        Some("image/jpeg") | Some("image/png") | Some("image/gif") | Some("image/webp") => {
          if let Ok(bytes) = attachment.download().await {
            if let Ok(bytes) = claude::util::resize_image(bytes, 600, 600) {
              items.push(Content::Image {
                source: ImageSource::Base64 {
                  media_type: content_type.unwrap().to_owned(),
                  data: BASE64_STANDARD.encode(&bytes),
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
}
