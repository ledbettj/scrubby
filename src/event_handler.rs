use std::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use serenity::{
  all::GuildId,
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Event {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Handler {
  tx: mpsc::UnboundedSender<Event>,
  running: AtomicBool,
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<Event>) -> Self {
    Self {
      tx,
      running: AtomicBool::new(false),
    }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, ctx: Context, ready: Ready) {
    println!("Ready and connected as {}", ready.user.name);
    let event = Event::ReadyEvent(ready, ctx);
    if let Err(e) = self.tx.send(event) {
      println!("error: {:?}", e);
    }
  }

  async fn message(&self, ctx: Context, msg: Message) {
    let event = Event::MessageEvent(msg, ctx);
    if let Err(e) = self.tx.send(event) {
      println!("error: {:?}", e);
    }
  }

  async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
    if !self.running.load(Ordering::Relaxed) {
      let ctx = ctx.clone();
      let tx = self.tx.clone();

      tokio::spawn(async move {
        println!("tick loop started");
        loop {
          tokio::time::sleep(Duration::from_secs(1)).await;
          let event = Event::TickEvent(ctx.clone());
          if let Err(e) = tx.send(event) {
            println!("error: {:?}", e);
          };
        }
      });

      self.running.swap(true, Ordering::Relaxed);
    }
  }
}
