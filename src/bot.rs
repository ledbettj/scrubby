use crate::event_handler::VoiceEvent;
use crate::plugins::PluginEnv;
use colored::Colorize;
use serenity::all::{Channel, ChannelType, GuildId};
use serenity::model::{channel::GuildChannel, channel::Message, gateway::Ready};
use serenity::prelude::*;
use sonata_synth::SonataSpeechSynthesizer;
use songbird::input::codecs::{CODEC_REGISTRY, PROBE};
use songbird::input::Input;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum BotEvent {
  MessageEvent(Message, Context),
  ReadyEvent(Ready, Context),
  TickEvent(Context),
}

pub struct Bot {
  captured_text: Vec<String>,
  plugin_env: PluginEnv,
}

impl Bot {
  fn new() -> Self {
    Self {
      captured_text: vec![],
      plugin_env: PluginEnv::new("./plugins"),
    }
  }

  pub async fn start(
    mut rx: mpsc::UnboundedReceiver<BotEvent>,
    vrx: mpsc::UnboundedReceiver<VoiceEvent>,
  ) -> () {
    let mut bot = Bot::new();

    let (tx, mut text_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    tokio::spawn(super::voice::recognizer(vrx, tx));

    let (synth, synth_rate) = super::voice::init_synth();

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
          bot.dispatch_event(&event, gid, &synth, synth_rate).await;
        },
        Some(text) = text_rx.recv() => {
          println!("{:?}", text);
          bot.captured_text.push(text);
        }
      }
    }
  }

  async fn dispatch_event(
    &mut self,
    event: &BotEvent,
    gid: Option<GuildId>,
    synth: &SonataSpeechSynthesizer,
    synth_rate: usize,
  ) -> () {
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
        if !self.captured_text.is_empty() {
          let mgr = songbird::get(ctx).await.expect("Voice client");
          if let Some(handler_lock) = mgr.get(gid.unwrap()) {
            let text = self.captured_text.iter().cloned().collect::<String>();
            self.captured_text.clear();
            println!("generating: \"{}\"", text);
            let output = super::voice::generate(synth, synth_rate, &text);
            let mut handler = handler_lock.lock().await;
            println!("playing!");
            let mut input: Input = output.into();
            input = input
              .make_playable_async(&CODEC_REGISTRY, &PROBE)
              .await
              .expect("Oh shit");
            handler.play_input(input);
          }
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
