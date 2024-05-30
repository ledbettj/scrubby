use std::collections::HashMap;

use colored::Colorize;
use serenity::all::{Channel, ChannelType};
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use tokio::sync::mpsc;

use super::llm::LLM;

use crate::plugins::PluginEnv;

#[derive(Debug)]
pub enum BotEvent {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Bot {
  plugin_env: PluginEnv,
  llm: LLM,
}

impl Bot {
  fn new() -> Self {
    Self {
      plugin_env: PluginEnv::new("./plugins"),
      llm: LLM::new(
        std::env::var("CLAUDE_KEY").expect("No CLAUDE_KEY provided"),
        vec![],
      ),
    }
  }

  pub async fn start(mut rx: mpsc::UnboundedReceiver<BotEvent>) -> () {
    let mut bot = Bot::new();

    if let Err(e) = bot.plugin_env.load(false) {
      println!("[{}] {}", "Error".red().bold(), e);
    }

    let tools = bot.plugin_env.tools().expect("OH SHIT");
    bot.llm.update_tools(tools);

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
          Self::process_reload_request(&msg, &ctx, &mut self.plugin_env).await;
          if let Ok(tools) = self.plugin_env.tools() {
            self.llm.update_tools(tools);
          }
          return;
        }

        let res = self.llm.respond(
          &msg.author.name,
          &msg.content,
          |name: &str, input: HashMap<String, String>| self.plugin_env.invoke_tool(name, input),
        );

        match res {
          Ok(s) => msg.reply(&ctx.http(), s).await,
          Err(e) => {
            msg
              .reply(&ctx.http(), format!("Error:\n```\n{}\n```", e))
              .await
          }
        }
        .map_err(|err| println!("[{}] Failed to reply: {}", "Error".red().bold(), err))
        .ok();

        // if let Ok(replies) = self.plugin_env.dispatch_message(&msg, &ctx) {
        //   for r in replies {
        //     match r {
        //       (Some(s), None) => {
        //         msg
        //           .reply(&ctx.http(), s)
        //           .await
        //           .map_err(|err| println!("[{}] Failed to reply: {}", "Error".red().bold(), err))
        //           .ok();
        //       }
        //       (None, Some(m)) => {
        //         msg
        //           .channel_id
        //           .send_message(ctx.http(), m)
        //           .await
        //           .map_err(|err| {
        //             println!("[{}] Failed to send message: {}", "Error".red().bold(), err)
        //           })
        //           .ok();
        //       }
        //       _ => {}
        //     }
        //   }
        // }
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
