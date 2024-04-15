use colored::Colorize;
use serenity::model::{channel::Message, gateway::Ready};
use serenity::prelude::*;
use tokio::sync::mpsc;

use crate::plugins::PluginEnv;

#[derive(Debug)]
pub enum BotEvent {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Bot;

impl Bot {
  pub async fn start(mut rx: mpsc::UnboundedReceiver<BotEvent>) -> () {
    let mut plugin_env = PluginEnv::new("./plugins");

    if let Err(e) = plugin_env.load(false) {
      println!("[{}] {}", "Error".red().bold(), e);
    }

    while let Some(event) = rx.recv().await {
      match &event {
        BotEvent::MessageEvent(msg, ctx) => {
          if !Self::message_is_respondable(&msg, &ctx).await {
            continue;
          }

          if msg.content.contains("reload") {
            Self::process_reload_request(&msg, &ctx, &mut plugin_env).await;
            continue;
          }

          if let Ok(replies) = plugin_env.dispatch_message(&msg, &ctx) {
            for r in replies {
              match r {
                (Some(s), None) => {
                  msg
                    .reply(&ctx.http(), s)
                    .await
                    .map_err(|err| println!("[{}] Failed to reply: {}", "Error".red().bold(), err))
                    .ok();
                }
                (None, Some(m)) => {
                  msg
                    .channel_id
                    .send_message(ctx.http(), m)
                    .await
                    .map_err(|err| {
                      println!("[{}] Failed to send message: {}", "Error".red().bold(), err)
                    })
                    .ok();
                }
                _ => {}
              }
            }
          }
        }
        BotEvent::ReadyEvent(ready, ctx) => {
          if let Err(err) = plugin_env.process_ready_event(&ready, &ctx) {
            println!("[{}] {}", "Error".red().bold(), err);
          }
        }
        BotEvent::TickEvent(ctx) => {
          if let Err(err) = plugin_env.process_tick_event(&ctx) {
            println!("[{}] {}", "Error".red().bold(), err);
          }
        }
      };
    }
  }

  async fn message_is_respondable(msg: &Message, ctx: &Context) -> bool {
    // dont respond to your own messages
    if msg.is_own(&ctx) {
      return false;
    }
    // always respond to private messages
    if msg.is_private() {
      return true;
    }

    // respond if you're mentioned
    if let Ok(is_mentioned) = msg.mentions_me(&ctx).await {
      is_mentioned
    } else {
      false
    }
  }

  async fn process_reload_request(msg: &Message, ctx: &Context, plugin_env: &mut PluginEnv) {
    match plugin_env.load(true) {
      Err(e) => {
        msg.react(&ctx.http(), '❌').await.expect("Failed to react");

        msg
          .reply(&ctx.http(), format!("```\n{}\n```", e))
          .await
          .expect("Failed to send reply");
      }
      Ok(_) => {
        msg.react(&ctx.http(), '✅').await.expect("Failed to react");
      }
    };
  }
}
