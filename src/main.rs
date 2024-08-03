use dotenv;
use serenity::prelude::*;
use std::env;
use tokio::sync::mpsc;

mod bot;
mod claude;
mod event_handler;
mod plugins;

use bot::Bot;
use env_logger::Builder;
use log::warn;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  Builder::new()
    .filter_module("scrubby", log::LevelFilter::Trace)
    .format_module_path(false)
    .init();

  if let Err(e) = dotenv::dotenv() {
    warn!("Failed to load .env file: {}", e);
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
