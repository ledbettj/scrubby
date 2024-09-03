use std::{cell::RefCell, collections::{HashMap, VecDeque}};

use log::{debug, error, info, trace};
use base64::prelude::*;

use serenity::{
  all::{Channel, ChannelId, ChannelType, GuildChannel, GuildId}, async_trait, builder::CreateMessage, model::{channel::Message, gateway::Ready}, prelude::*
};

use crate::plugins::Host;
use crate::claude::{Content, Client, Role, Interaction, ImageSource, Response};
pub enum BotResponse {
  Text(String),
  Embedded(CreateMessage),
}

pub struct Handler {
  host: Host,
  claude: Client,
  history: HashMap<ChannelId, VecDeque<Interaction>>,
}

impl Handler {
  pub fn new(host: Host, claude_key: &str) -> Self {
    Self {
      host,
      claude: Client::new(
        claude_key,
        include_str!("./claude/prompt.txt"),
        crate::claude::Model::Sonnet35
      ),
      history: HashMap::new(),
    }
  }

  async fn message_is_respondable(msg: &Message, ctx: &Context) -> bool {
    // dont respond to your own messages
    if msg.author.id == ctx.cache.current_user().id {
      return false;
    }

    // dont respond to blank messages
    if msg.content_safe(ctx).trim().is_empty() && msg.attachments.is_empty() {
      return false;
    }

    // always respond to private messages
    if msg.guild_id.is_none() {
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

      let resp = claude.create_message(&history.make_contiguous(), host).await;
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

    // otherwise try parsing each one individually
    Ok(
      output
        .into_iter()
        .map(|text| BotResponse::Text(text))
        .collect()
    )
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, ctx: Context, ready: Ready) {
    info!("connected as {}", ready.user.name,);
  }

  async fn message(&self, ctx: Context, msg: Message) {
    debug!("{:?}", msg);

    let respondable = Self::message_is_respondable(&msg, &ctx).await;
    let msg_content = Self::msg_to_content(&msg, &ctx).await;

    if msg_content.is_empty() {
      return;
    }

    let mut history = self.history.entry(msg.channel_id).or_default();
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

    if !respondable {
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

    let replies = match Self::dispatch_llm(&mut history, &self.claude, &self.host).await {
      Ok(replies) => replies,
      Err(e) => {
        error!("{}", e);

        history.iter().for_each(|item| trace!("{:?}", item));

        vec![BotResponse::Text(
          format!(":skull:\n```\n{}\n```", e).to_owned(),
        )]
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

  async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
  }
}
