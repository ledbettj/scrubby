use colored::Colorize;
use serenity::all::{Channel, ChannelType, GuildId};
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use songbird::input::codecs::{CODEC_REGISTRY, PROBE};
use songbird::input::Input;
use songbird::model::id::UserId;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
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
  voice_data: Vec<i16>,
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
    storage: &mut HashMap<UserId, VecDeque<i16>>,
  ) -> () {
    match event {
      VoiceEvent::Data(uid, data) => {
        println!("{:?} speaking {:?}", uid, data.len());

        // let bytes = data.iter().flat_map(|val| [(val & 0xFF) as u8, (val >> 8) as u8]).collect::<Vec<u8>>();
        // stream.write_all(&bytes).expect("Failed to write to stdout");

        storage
          .entry(uid)
          .and_modify(|samples| samples.extend(&data))
          .or_insert_with(|| data.into());
      }
      VoiceEvent::Silent => {
        println!("all quiet {:?}", storage);
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
    let mut map: HashMap<_, VecDeque<i16>> = HashMap::new();

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
        println!("data len {:?}", self.voice_data.len());
        if !self.voice_data.is_empty() {
          let mgr = songbird::get(ctx).await.expect("Voice client");
          println!("play time");
          if let Some(handler_lock) = mgr.get(gid.unwrap()) {
            let mut handler = handler_lock.lock().await;
            let fsize = 44 + self.voice_data.len() * 2;
            let dsize = self.voice_data.len() * 2;
            let rate = 48000;
            let mul = rate * 16 * 2 / 8;
            let mut data = vec![
              'R' as u8,
              'I' as u8,
              'F' as u8,
              'F' as u8,
              (fsize & 0xFF) as u8,
              (fsize >> 8) as u8,
              (fsize >> 16) as u8,
              (fsize >> 24) as u8,
              'W' as u8,
              'A' as u8,
              'V' as u8,
              'E' as u8,
              'f' as u8,
              'm' as u8,
              't' as u8,
              ' ' as u8,
              16,
              0,
              0,
              0,
              1,
              0,
              2,
              0,
              (rate & 0xFF) as u8,
              (rate >> 8) as u8,
              (rate >> 16) as u8,
              (rate >> 24) as u8,
              (mul & 0xFF) as u8,
              (mul >> 8) as u8,
              (mul >> 16) as u8,
              (mul >> 24) as u8,
              (16 * 2) / 8 as u8,
              0,
              16,
              0,
              'd' as u8,
              'a' as u8,
              't' as u8,
              'a' as u8,
              (dsize & 0xFF) as u8,
              (dsize >> 8) as u8,
              (dsize >> 16) as u8,
              (dsize >> 24) as u8,
            ];
            data.extend(
              self
                .voice_data
                .iter()
                .flat_map(|short| [(short & 0xFF) as u8, (short >> 8) as u8]),
            );
            println!("playing!");
            let mut input: Input = data.into();
            input = input
              .make_playable_async(&CODEC_REGISTRY, &PROBE)
              .await
              .expect("Oh shit");
            println!("{:?}", input.is_playable());
            println!("{:?}", handler.play_input(input));
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
