use event_dispatch::event_dispatch;
use serenity::prelude::*;
use tokio::sync::mpsc;

mod bindings;
mod event_dispatch;
mod event_handler;
mod lua_context;
mod lua_loader;
mod user_data;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::GUILDS
    | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let handler = event_handler::Handler::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await?;

  tokio::spawn(event_dispatch(rx));

  client.start().await?;

  Ok(())
}
