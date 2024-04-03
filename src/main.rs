use std::{fs::File, io::Read};

use mlua::Lua;
use serenity::{prelude::*, async_trait, model::{channel::Message, gateway::Ready}};
use tokio::sync::mpsc;

pub struct Handler {
  tx: mpsc::UnboundedSender<(Message, Context)>,
}

impl Handler {
  pub fn new(tx: mpsc::UnboundedSender<(Message, Context)>) -> Self {
    Self { tx }
  }
}

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, _: Context, ready: Ready) {
    println!("Ready and connected as {}", ready.user.name);
  }

  async fn message(&self, ctx: Context, msg: Message) {
    if let Err(e) = self.tx.send((msg, ctx)) {
      println!("error: {:?}", e);
    }
  }
}

async fn lua_loop(mut rx: mpsc::UnboundedReceiver<(Message, Context)>) -> () {
  let lua = Lua::new();

  {
    let bot = lua.create_table().expect("lua: Failed to create Bot object");
    let plugin = lua.create_function(|l: &Lua, name: String| {
      let tbl = l.create_table()?;
      let def = l.create_function(|_, ()| Ok(()) )?;
      tbl.set("name", name)?;
      tbl.set("on_message", def)?;
      Ok(tbl)
    }).expect("lua: Failed to create Bot.plugin");

    let plugins = lua.create_table().expect("lua: Failed to create plugins table");
    let register = lua.create_function(|l: &Lua, tbl: mlua::Table| {
      let bot : mlua::Table = l.globals().get("Bot")?;
      let name : String = tbl.get("name")?;
      let plugins : mlua::Table = bot.get("plugins")?;
      plugins.set(name, tbl)?;
      Ok(())
    }).expect("lua: Failed to create Bot.register");

    bot.set("plugin", plugin).expect("lua: Failed to set Bot.plugin");
    bot.set("plugins", plugins).expect("lua: Failed to set Bot.plugins");
    bot.set("register", register).expect("lua: Failed to set Bot.register");

    lua.globals().set("Bot", bot).expect("lua: Failed to set Bot object");
  }

  for script in std::fs::read_dir("./scripts").unwrap() {
    let script = script.unwrap();
    let mut f = File::open(script.path()).unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    lua.load(buf).exec().unwrap();
    println!("loaded {:?}", script.path());
  }

  while let Some((msg, ctx)) = rx.recv().await {
    if msg.is_own(&ctx) {
      continue;
    }

    if !msg.is_private() {
      if let Ok(false) = msg.mentions_me(&ctx).await {
        continue;
      }
    }

    let mut replies = vec![];

    {
      let bot : mlua::Table = lua.globals().get("Bot").expect("lua: Failed to locate Bot object");
      let plugins : mlua::Table = bot.get("plugins").expect("lua: Failed to locate Bot.plugins");
      let m = lua.create_table().expect("lua: Failed to create message object");
      m.set("id", msg.id.get()).expect("lua: Failed to create message object");
      m.set("author", msg.author.name.clone()).expect("lua: Failed to create message object");
      m.set("content", msg.content.clone()).expect("lua: Failed to to create message object");

      plugins.for_each::<String, mlua::Table>(|name, v|{
        let f : mlua::Function = v.get("on_message")?;
        match f.call::<&mlua::Table, Option<String>>(&m) {
          Ok(None) => { /* no op */ },
          Ok(Some(s)) => replies.push(s),
          Err(e) => println!("lua: error dispatching event to {}: {}", name, e),
        };
        Ok(())
      }).expect("lua: Failed to iterate over plugins");
    }

    for r in replies {
      msg.reply(&ctx.http, r).await.expect("oh no");
    }
  };
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN is not set");
  let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

  let (tx, rx) = mpsc::unbounded_channel();
  let handler = Handler::new(tx);
  let mut client = Client::builder(&token, intents)
    .event_handler(handler)
    .await?;

  tokio::spawn(lua_loop(rx));

  client.start().await?;

  Ok(())
}
