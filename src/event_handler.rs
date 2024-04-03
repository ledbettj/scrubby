use serenity::{
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Event {
  MessageEvent(Message, Context),
  ReadyEvent(Ready),
}

pub struct Handler {
  tx: mpsc::UnboundedSender<Event>,
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<Event>) -> Self {
    Self { tx }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, _: Context, ready: Ready) {
    println!("Ready and connected as {}", ready.user.name);
    let event = Event::ReadyEvent(ready);
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
}
