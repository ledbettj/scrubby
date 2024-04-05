use serenity::model::channel::Message;
use serenity::prelude::{CacheHttp, Context};

use tokio::sync::mpsc;

use crate::event_handler::Event;
use crate::lua_context::LuaContext;

pub async fn event_dispatch(mut rx: mpsc::UnboundedReceiver<Event>) -> () {
  let mut lua_ctx = LuaContext::new("./scripts");

  if let Err(e) = lua_ctx.load_plugins(false) {
    println!("Error loading plugins: {}", e);
  }

  while let Some(event) = rx.recv().await {
    match &event {
      Event::MessageEvent(msg, ctx) => {
        if !message_is_respondable(&msg, &ctx).await {
          continue;
        }

        if msg.content.contains("reload") {
          process_reload_request(&msg, &ctx, &mut lua_ctx).await;
          continue;
        }

        if let Ok(replies) = lua_ctx.dispatch_message(&msg, &ctx) {
          for r in replies {
            msg
              .reply(&ctx.http(), r)
              .await
              .expect("Failed to send reply");
          }
        }
      }
      Event::ReadyEvent(ready) => {
        if let Err(err) = lua_ctx.process_ready_event(&ready) {
          println!("ReadyEvent error: {:?}", err);
        }
      }
    };
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

  // respond if you're mentioned
  if let Ok(is_mentioned) = msg.mentions_me(&ctx).await {
    is_mentioned
  } else {
    false
  }
}

async fn process_reload_request(msg: &Message, ctx: &Context, lua_ctx: &mut LuaContext) {
  match lua_ctx.load_plugins(true) {
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