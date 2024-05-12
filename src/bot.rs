use colored::Colorize;
use serenity::all::{Channel, ChannelType, GuildId};
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use songbird::input::codecs::{CODEC_REGISTRY, PROBE};
use songbird::input::Input;
use songbird::model::id::UserId;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::event_handler::VoiceEvent;
use crate::plugins::PluginEnv;

#[derive(Debug)]
pub enum BotEvent {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Bot {
  voice_data: Vec<f32>,
  plugin_env: PluginEnv,
}

impl Bot {
  fn new() -> Self {
    Self {
      voice_data: vec![],
      plugin_env: PluginEnv::new("./plugins"),
    }
  }

  pub async fn dispatch_voice_event(
    &mut self,
    event: VoiceEvent,
    storage: &mut HashMap<UserId, Vec<f32>>,
  ) -> () {
    match event {
      VoiceEvent::Data(uid, data) => {
        println!("{:?} speaking {:?}", uid, data.len());

        storage
          .entry(uid)
          .and_modify(|samples| samples.extend(&data))
          .or_insert_with(|| data.into());
      }
      VoiceEvent::Silent => {
        self.voice_data.clear();
        storage
          .iter()
          .for_each(|(_id, data)| self.voice_data.extend(data));
        storage.clear();
      }
    }
  }

  pub async fn start(
    mut rx: mpsc::UnboundedReceiver<BotEvent>,
    mut vrx: mpsc::UnboundedReceiver<VoiceEvent>,
  ) -> () {
    let mut bot = Bot::new();
    let mut map: HashMap<_, Vec<f32>> = HashMap::new();

    if let Err(e) = bot.plugin_env.load(false) {
      println!("[{}] {}", "Error".red().bold(), e);
    }
    let mut gid = None;

    loop {
      tokio::select! {
        Some(event) = rx.recv() => {
          if let BotEvent::ReadyEvent(ref ready, _) = event {
            gid = Some(ready.guilds[0].id);
          }
          bot.dispatch_event(&event, gid).await;
        },
        Some(event) = vrx.recv() => {
          bot.dispatch_voice_event(event, &mut map).await;
        }
      }
    }
  }

  async fn dispatch_event(&mut self, event: &BotEvent, gid: Option<GuildId>) -> () {
    match event {
      BotEvent::MessageEvent(msg, ctx) => {
        if !Self::message_is_respondable(&msg, &ctx).await {
          return;
        }

        if msg.content.contains("reload") {
          Self::process_reload_request(&msg, &ctx, &mut self.plugin_env).await;
          return;
        }

        if let Ok(replies) = self.plugin_env.dispatch_message(&msg, &ctx) {
          for r in replies {
            match r {
              (Some(s), None) => {
                msg
                  .reply(&ctx.http(), s)
                  .await
                  .map_err(|err| println!("[{}] Failed to reply: {}", "Error".red().bold(), err))
                  .ok();
              }
              (None, Some(m)) => {
                msg
                  .channel_id
                  .send_message(ctx.http(), m)
                  .await
                  .map_err(|err| {
                    println!("[{}] Failed to send message: {}", "Error".red().bold(), err)
                  })
                  .ok();
              }
              _ => {}
            }
          }
        }
      }
      BotEvent::ReadyEvent(ready, ctx) => {
        if let Err(err) = self.plugin_env.process_ready_event(&ready, &ctx) {
          println!("[{}] {}", "Error".red().bold(), err);
        }
      }
      BotEvent::TickEvent(ctx) => {
        if !self.voice_data.is_empty() {
          let mgr = songbird::get(ctx).await.expect("Voice client");
          println!("play time");
          if let Some(handler_lock) = mgr.get(gid.unwrap()) {
            let text = super::voice::recognize(&self.voice_data);
            let output = super::voice::generate(&text);
            let mut handler = handler_lock.lock().await;
            println!("playing!");
            let mut input: Input = output.into();
            input = input
              .make_playable_async(&CODEC_REGISTRY, &PROBE)
              .await
              .expect("Oh shit");
            handler.play_input(input);
          }
          self.voice_data.clear();
        }

        if let Err(err) = self.plugin_env.process_tick_event(&ctx) {
          println!("[{}] {}", "Error".red().bold(), err);
        }
      }
    }
  }

  async fn message_is_respondable(msg: &Message, ctx: &Context) -> bool {
    // dont respond to your own messages
    if msg.is_own(&ctx) {
      return false;
    }
    // always respond to private messages
    if msg.is_private() {
      return true;
    }

    // respond if it's a thread we're involved in.
    let channel = msg.channel(ctx.http()).await;
    if let Ok(Channel::Guild(GuildChannel {
      kind: ChannelType::PublicThread,
      member: Some(_),
      ..
    })) = channel
    {
      return true;
    }

    // respond if you're mentioned
    if let Ok(is_mentioned) = msg.mentions_me(&ctx).await {
      is_mentioned
    } else {
      false
    }
  }

  async fn process_reload_request(msg: &Message, ctx: &Context, plugin_env: &mut PluginEnv) {
    match plugin_env.load(true) {
      Err(e) => {
        msg.react(&ctx.http(), '❌').await.expect("Failed to react");

        msg
          .reply(&ctx.http(), format!("```\n{}\n```", e))
          .await
          .expect("Failed to send reply");
      }
      Ok(_) => {
        msg.react(&ctx.http(), '✅').await.expect("Failed to react");
      }
    };
  }
}
