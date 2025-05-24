use log::{debug, info};
use serenity::{
  all::{GuildChannel, GuildId},
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc::UnboundedSender;

/// Wrapper for Discord message events with associated context.
/// Contains the message data and Discord API context needed for bot responses.
#[derive(Debug)]
pub struct MsgEvent {
  pub ctx: Context,
  pub msg: Message,
}

/// Event fired when the bot successfully connects to Discord.
/// Contains the list of guilds the bot has access to for initialization.
#[derive(Debug)]
pub struct ReadyEvent {
  pub ctx: Context,
  pub guilds: Vec<GuildId>,
}

/// Event fired when a Discord thread is modified.
/// Used to clean up bot state when threads are archived or deleted.
#[derive(Debug)]
pub struct ThreadUpdateEvent {
  pub ctx: Context,
  pub old: Option<GuildChannel>,
  pub new: GuildChannel,
}

/// All Discord events that the bot processes.
/// These events are forwarded from the Discord gateway to the main event handler.
#[derive(Debug)]
pub enum BotEvent {
  Message(MsgEvent),
  Ready(ReadyEvent),
  ThreadUpdate(ThreadUpdateEvent),
}

/// Bridges Discord's event system with the bot's internal event processing.
/// Receives Discord events and forwards them via channels to the main handler.
pub struct EventDispatcher {
  tx: UnboundedSender<BotEvent>,
}

impl EventDispatcher {
  /// Creates a new event dispatcher that forwards events to the given channel.
  /// The channel sender is used to decouple Discord event handling from bot logic.
  pub fn new(tx: UnboundedSender<BotEvent>) -> Self {
    Self { tx }
  }
}

#[async_trait]
impl EventHandler for EventDispatcher {
  /// Handles the Discord ready event when the bot connects successfully.
  /// Forwards guild information to the main handler for configuration setup.
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

  /// Handles Discord thread update events for cleanup purposes.
  /// Forwards thread state changes so the bot can clean up conversation history.
  async fn thread_update(&self, ctx: Context, old: Option<GuildChannel>, new: GuildChannel) {
    let event = BotEvent::ThreadUpdate(ThreadUpdateEvent { ctx, old, new });
    self
      .tx
      .send(event)
      .expect("Failed to write thread update to channel");
  }

  /// Handles incoming Discord messages and forwards them for processing.
  /// All message events pass through here before reaching the main bot logic.
  async fn message(&self, ctx: Context, msg: Message) {
    debug!("{:?}", msg);

    let event = BotEvent::Message(MsgEvent { ctx, msg });
    self
      .tx
      .send(event)
      .expect("Failed to write message content to channel");
  }

  /// Handles Discord cache ready events (currently unused).
  /// Required by the EventHandler trait but not needed for bot functionality.
  async fn cache_ready(&self, _ctx: Context, _guilds: Vec<GuildId>) {}
}
