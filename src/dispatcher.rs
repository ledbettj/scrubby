use log::{debug, info};
use serenity::{
  all::GuildId,
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct BotEvent {
  pub ctx: Context,
  pub msg: Message,
}

pub struct EventDispatcher {
  tx: UnboundedSender<BotEvent>,
}

impl EventDispatcher {
  pub fn new(tx: UnboundedSender<BotEvent>) -> Self {
    Self { tx }
  }
}

#[async_trait]
impl EventHandler for EventDispatcher {
  async fn ready(&self, _ctx: Context, ready: Ready) {
    info!("connected as {}", ready.user.name);
  }

  async fn message(&self, ctx: Context, msg: Message) {
    debug!("{:?}", msg);

    let event = BotEvent { ctx, msg };
    self
      .tx
      .send(event)
      .expect("Failed to write message content to channel");
  }

  async fn cache_ready(&self, _ctx: Context, _guilds: Vec<GuildId>) {}
}
