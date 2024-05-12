use serenity::prelude::*;
use tokio::sync::mpsc;

use songbird::{driver::DecodeMode, SerenityInit};

mod bot;
mod event_handler;
mod plugins;

use bot::Bot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let (vtx, vrx) = mpsc::unbounded_channel();

  let handler = event_handler::Handler::new(tx, vtx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .register_songbird_from_config(songbird::Config::default().decode_mode(DecodeMode::Decode))
    .await?;

  tokio::spawn(Bot::start(rx, vrx));

  client.start().await?;

  Ok(())
}
