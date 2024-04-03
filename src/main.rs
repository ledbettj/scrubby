use std::{fs::File, io::Read};

use mlua::Lua;
use serenity::{
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc;

mod lua;

pub struct Handler {
  tx: mpsc::UnboundedSender<(Message, Context)>,
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<(Message, Context)>) -> Self {
    Self { tx }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, _: Context, ready: Ready) {
    println!("Ready and connected as {}", ready.user.name);
  }

  async fn message(&self, ctx: Context, msg: Message) {
    if let Err(e) = self.tx.send((msg, ctx)) {
      println!("error: {:?}", e);
    }
  }
}

async fn lua_loop(mut rx: mpsc::UnboundedReceiver<(Message, Context)>) -> () {
  let mut lua_ctx = lua::LuaContext::new("./scripts");

  if let Err(e) = lua_ctx.load_plugins() {
    println!("Error loading plugins: {}", e);
  }

  while let Some((msg, ctx)) = rx.recv().await {
    if msg.is_own(&ctx) {
      continue;
    }

    if !msg.is_private() {
      if let Ok(false) = msg.mentions_me(&ctx).await {
        continue;
      }
    }
    if msg.content.contains("reload") {
      match lua_ctx.load_plugins() {
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
      continue;
    }

    if let Ok(replies) = lua_ctx.dispatch_message(&msg, &ctx) {
      for r in replies {
        msg
          .reply(&ctx.http(), r)
          .await
          .expect("Failed to send reply");
      }
    }
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let handler = Handler::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await?;

  tokio::spawn(lua_loop(rx));

  client.start().await?;

  Ok(())
}
