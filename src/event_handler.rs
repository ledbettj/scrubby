use std::{
  sync::atomic::{AtomicBool, Ordering},
  sync::Arc,
  time::Duration,
};

use crate::bot::BotEvent;
use colored::Colorize;
use dashmap::DashMap;
use serenity::{
  all::{ChannelType, GuildId},
  async_trait,
  model::{channel::Message, gateway::Ready},
  prelude::*,
};
use songbird::{
  events::{CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler},
  model::id::UserId,
  model::payload::{ClientDisconnect, Speaking},
};
use tokio::sync::mpsc;

pub enum VoiceEvent {
  Data(Option<UserId>, Vec<f32>),
  Silent,
}

pub struct Handler {
  tx: mpsc::UnboundedSender<BotEvent>,
  vtx: mpsc::UnboundedSender<VoiceEvent>,
  running: AtomicBool,
}

struct InnerVoiceHandler {
  last_tick_empty: AtomicBool,
  known_ssrcs: DashMap<u32, UserId>,
}

#[derive(Clone)]
pub struct VoiceHandler {
  inner: Arc<InnerVoiceHandler>,
  tx: mpsc::UnboundedSender<VoiceEvent>,
}

impl VoiceHandler {
  pub fn new(tx: mpsc::UnboundedSender<VoiceEvent>) -> Self {
    Self {
      tx,
      inner: Arc::new(InnerVoiceHandler {
        last_tick_empty: AtomicBool::default(),
        known_ssrcs: DashMap::new(),
      }),
    }
  }
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<BotEvent>, vtx: mpsc::UnboundedSender<VoiceEvent>) -> Self {
    Self {
      tx,
      vtx,
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

    let guild = ready.guilds[0];
    if let Ok(channels) = ctx.http.get_channels(guild.id).await {
      let mgr = songbird::get(&ctx)
        .await
        .expect("Where my clients at")
        .clone();
      for channel in channels
        .iter()
        .filter(|channel| channel.kind == ChannelType::Voice)
      {
        if let Ok(handler_lock) = mgr.join(guild.id, channel.id).await {
          let mut handler = handler_lock.lock().await;
          let evt_receiver = VoiceHandler::new(self.vtx.clone());

          handler.add_global_event(CoreEvent::SpeakingStateUpdate.into(), evt_receiver.clone());
          handler.add_global_event(CoreEvent::ClientDisconnect.into(), evt_receiver.clone());
          handler.add_global_event(CoreEvent::VoiceTick.into(), evt_receiver);
        }
      }
    }

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

#[async_trait]
impl VoiceEventHandler for VoiceHandler {
  async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
    use EventContext as Ctx;
    match ctx {
      Ctx::SpeakingStateUpdate(Speaking {
        speaking,
        ssrc,
        user_id,
        ..
      }) => {
        // Discord voice calls use RTP, where every sender uses a randomly allocated
        // *Synchronisation Source* (SSRC) to allow receivers to tell which audio
        // stream a received packet belongs to. As this number is not derived from
        // the sender's user_id, only Discord Voice Gateway messages like this one
        // inform us about which random SSRC a user has been allocated. Future voice
        // packets will contain *only* the SSRC.
        //
        // You can implement logic here so that you can differentiate users'
        // SSRCs and map the SSRC to the User ID and maintain this state.
        // Using this map, you can map the `ssrc` in `voice_packet`
        // to the user ID and handle their audio packets separately.
        println!(
          "Speaking state update: user {:?} has SSRC {:?}, using {:?}",
          user_id, ssrc, speaking,
        );

        if let Some(user) = user_id {
          self.inner.known_ssrcs.insert(*ssrc, *user);
        }
      }
      Ctx::VoiceTick(tick) => {
        let speaking = tick.speaking.len();
        let total_participants = speaking + tick.silent.len();
        let last_tick_empty = self.inner.last_tick_empty.load(Ordering::SeqCst);

        if speaking == 0 && !last_tick_empty {
          self
            .tx
            .send(VoiceEvent::Silent)
            .expect("Failed to write data");

          self.inner.last_tick_empty.store(true, Ordering::SeqCst);
        } else if speaking != 0 {
          self.inner.last_tick_empty.store(false, Ordering::SeqCst);

          //println!("Voice tick ({speaking}/{total_participants} live):");

          // You can also examine tick.silent to see users who are present
          // but haven't spoken in this tick.
          for (ssrc, data) in &tick.speaking {
            let user_id = match self.inner.known_ssrcs.get(&ssrc) {
              Some(v) => Some(v.clone()),
              None => None,
            };

            // This field should *always* exist under DecodeMode::Decode.
            // The `else` allows you to see how the other modes are affected.
            if let Some(decoded_voice) = data.decoded_voice.as_ref() {
              let converted = super::voice::convert_pcm(&decoded_voice, 48000, 2, 16000);
              self
                .tx
                .send(VoiceEvent::Data(user_id, converted))
                .expect("Failed to write data");
            }
          }
        }
      }
      Ctx::ClientDisconnect(ClientDisconnect { user_id, .. }) => {
        // You can implement your own logic here to handle a user who has left the
        // voice channel e.g., finalise processing of statistics etc.
        // You will typically need to map the User ID to their SSRC; observed when
        // first speaking.

        println!("Client disconnected: user {:?}", user_id);
      }
      _ => {
        // We won't be registering this struct for any more event classes.
        unimplemented!()
      }
    }
    None
  }
}
