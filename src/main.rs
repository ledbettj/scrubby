use dotenv;
use env_logger::Builder;
use log::warn;
use serenity::prelude::*;
use std::env;
use tokio::sync::mpsc;

mod channel;
mod claude;
mod dispatcher;
mod handler;
mod storage;

use dispatcher::{BotEvent, EventDispatcher};
use handler::EventHandler;

pub const PROMPT_TEMPLATE: &'static str = include_str!("./claude/prompt.txt");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  Builder::new()
    .filter_module("scrubby2", log::LevelFilter::Trace)
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

  let (tx, rx) = mpsc::unbounded_channel::<BotEvent>();

  let dispatcher = EventDispatcher::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(dispatcher)
    .await?;

  tokio::spawn(async move { EventHandler::start("./storage", &claude_key, rx).await });

  client.start().await?;

  Ok(())
}
