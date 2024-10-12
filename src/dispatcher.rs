use log::{debug, info};
use serenity::{
  all::GuildId,
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct MsgEvent {
  pub ctx: Context,
  pub msg: Message,
}

#[derive(Debug)]
pub struct ReadyEvent {
  pub ctx: Context,
  pub guilds: Vec<GuildId>,
}

#[derive(Debug)]
pub enum BotEvent {
  Message(MsgEvent),
  Ready(ReadyEvent),
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
  async fn ready(&self, ctx: Context, ready: Ready) {
    info!("connected as {}", ready.user.name);
    let event = BotEvent::Ready(ReadyEvent {
      guilds: ready.guilds.iter().map(|g| g.id).collect(),
      ctx,
    });
    self
      .tx
      .send(event)
      .expect("Failed to write ready content to channel");
  }

  async fn message(&self, ctx: Context, msg: Message) {
    debug!("{:?}", msg);

    let event = BotEvent::Message(MsgEvent { ctx, msg });
    self
      .tx
      .send(event)
      .expect("Failed to write message content to channel");
  }

  async fn cache_ready(&self, _ctx: Context, _guilds: Vec<GuildId>) {}
}
