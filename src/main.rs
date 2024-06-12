use dotenv;
use serenity::prelude::*;
use std::env;
use tokio::sync::mpsc;

mod bot;
mod claude;
mod event_handler;
mod plugins;

use bot::Bot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  if let Err(e) = dotenv::dotenv() {
    println!("Warning: failed to load .env file: {}", e);
  }

  let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let claude_key = env::var("CLAUDE_KEY").expect("No CLAUDE_KEY provided");
  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::GUILDS
    | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let handler = event_handler::Handler::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await?;

  tokio::spawn(async move { Bot::start("./plugins", &claude_key, rx).await });

  client.start().await?;

  Ok(())
}
