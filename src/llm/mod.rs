use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use ureq;

mod content;
mod error;
mod schema;

pub use content::Content;
pub use error::Error;
pub use schema::Schema;

const CLAUDE_URL: &'static str = "https://api.anthropic.com/v1/messages";
const CLAUDE_PROMPT: &'static str = "
You are a general purpose informational assistant.  Your name is Scrubby.
Your responses should be brief, informative, and limited to about the size of a tweet if possible.
Don't be afraid to be rude or sarcastic if necessary -- that's part of your charm.

Only use the the provided tools if the user asks a question that matches the purpose of a tool.
If you can answer the question or respond without using a tool, do not use a tool.  Make sure to include
the tool output in your response if you invoke a tool.
";

pub struct LLM {
  pub history: HashMap<String, VecDeque<Interaction>>,
  pub api_key: String,
  pub tools: Vec<Tool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interaction {
  role: &'static str,
  content: Vec<Content>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tool {
  pub name: String,
  pub description: String,
  pub input_schema: Schema,
}

#[derive(Serialize)]
struct Request<'a> {
  model: &'static str,
  max_tokens: usize,
  system: String,
  messages: Vec<Interaction>,
  tools: &'a [Tool],
}

#[derive(Deserialize, Debug)]
pub struct Usage {
  input_tokens: usize,
  output_tokens: usize,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeResponse {
  id: String,
  model: String,
  role: String,
  stop_reason: Option<String>,
  stop_sequence: Option<String>,
  usage: Usage,
  content: Vec<Content>,
}

impl LLM {
  pub fn new<S: Into<String>>(api_key: S, tools: Vec<Tool>) -> Self {
    Self {
      api_key: api_key.into(),
      history: HashMap::new(),
      tools,
    }
  }

  pub fn update_tools(&mut self, tools: Vec<Tool>) {
    println!("tools: {:?}", &tools);
    self.tools = tools;
  }

  fn request(&self, messages: &VecDeque<Interaction>) -> Result<ureq::Response, ureq::Error> {
    println!("[{}] {:?}", "Debug".white().bold(), messages);

    let payload = Request {
      model: "claude-3-sonnet-20240229", //"claude-3-haiku-20240307",
      max_tokens: 1024,
      system: CLAUDE_PROMPT.into(),
      messages: messages.clone().into(),
      tools: &self.tools,
    };
    let body = serde_json::to_string(&payload).unwrap();
    ureq::post(CLAUDE_URL)
      .set("Content-Type", "application/json")
      .set("X-API-Key", &self.api_key)
      .set("Anthropic-Version", "2023-06-01")
      .set("Anthropic-Beta", "tools-2024-05-16")
      .send_string(&body)
  }

  pub fn respond<F: Fn(&str, HashMap<String, String>) -> anyhow::Result<Option<String>>>(
    &mut self, // msg: &Message
    author: &str,
    content: &str,
    invoke_tool: F,
  ) -> Result<String, Error> {
    let author = author.to_owned();
    let mut interaction = Interaction {
      role: "user",
      content: vec![Content::Text {
        text: content.to_owned(),
      }],
    };

    let mut output: Vec<String> = vec![];
    let mut done = false;

    while !done {
      println!("[{}] {:?}", "Debug".white().bold(), interaction);

      self
        .history
        .entry(author.clone())
        .or_default()
        .push_back(interaction.clone());

      println!("[{}] {:?}", "Debug".white().bold(), "history pushed");
      let messages = self.history.get(&author).unwrap();
      let resp = match self.request(&messages) {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) if code >= 400 && code < 500 => resp,
        Err(e) => return Err(Error::HttpError(e)),
      };

      let s = resp.into_string()?;
      println!("[{}] {:?}", "Debug".white().bold(), s);
      let c: ClaudeResponse = serde_json::from_str(&s)?;
      println!("[{}] {:?}", "Debug".white().bold(), c);

      done = true;

      self
        .history
        .get_mut(&author)
        .unwrap()
        .push_back(Interaction {
          role: "assistant",
          content: c.content.clone(),
        });

      for content in c.content.into_iter() {
        match content {
          Content::Text { text } => {
            output.push(text.clone());
          }
          // the LLM should never respond with an image or tool result.
          // those are input only types.
          Content::Image { .. } => unreachable!(),
          Content::ToolResult { .. } => unreachable!(),
          Content::ToolUse { id, name, input } => {
            done = false;

            let tool_content = match invoke_tool(&name, input) {
              Err(e) => Content::ToolResult {
                tool_use_id: id,
                content: e.to_string(),
                is_error: true,
              },
              Ok(None) => Content::ToolResult {
                tool_use_id: id,
                content: "<no output>".into(),
                is_error: false,
              },
              Ok(Some(s)) => Content::ToolResult {
                tool_use_id: id,
                content: s.into(),
                is_error: false,
              },
            };
            interaction = Interaction {
              role: "user",
              content: vec![tool_content],
            }
          }
        }
      }
    }

    Ok(output.join("\n"))
  }

  pub fn trim(&mut self) {
    for (_, v) in self.history.iter_mut() {
      if v.len() > 10 {
        let remove = v.len() - 10;
        v.drain(0..remove);
      }
    }
  }
}
