use std::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use colored::Colorize;
use serenity::{
  all::GuildId,
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc;

use crate::bot::BotEvent;

pub struct Handler {
  tx: mpsc::UnboundedSender<BotEvent>,
  running: AtomicBool,
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<BotEvent>) -> Self {
    Self {
      tx,
      running: AtomicBool::new(false),
    }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, ctx: Context, ready: Ready) {
    println!(
      "[{}] connected as {}",
      "Bot".yellow().bold(),
      ready.user.name
    );

    let event = BotEvent::ReadyEvent(ready, ctx);
    if let Err(e) = self.tx.send(event) {
      println!("[{}] {}", "Error".red().bold(), e);
    }
  }

  async fn message(&self, ctx: Context, msg: Message) {
    let event = BotEvent::MessageEvent(msg, ctx);
    if let Err(e) = self.tx.send(event) {
      println!("[{}] {}", "Error".red().bold(), e);
    }
  }

  async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
    if !self.running.load(Ordering::Relaxed) {
      let ctx = ctx.clone();
      let tx = self.tx.clone();

      tokio::spawn(async move {
        println!("[{}] {}", "Bot".yellow().bold(), "event loop initialized");
        loop {
          tokio::time::sleep(Duration::from_secs(1)).await;
          let event = BotEvent::TickEvent(ctx.clone());
          if let Err(e) = tx.send(event) {
            println!("[{}] {}", "Error".red().bold(), e);
          };
        }
      });

      self.running.swap(true, Ordering::Relaxed);
    }
  }
}
