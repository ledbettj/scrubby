use dotenv;
use serenity::prelude::*;
use std::env;
use tokio::sync::mpsc;

mod bot;
mod event_handler;
mod llm;
mod plugins;

use bot::Bot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  dotenv::dotenv().expect("Failed to load .env file");
  let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::GUILDS
    | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let handler = event_handler::Handler::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await?;

  tokio::spawn(Bot::start(rx));

  client.start().await?;

  Ok(())
}
