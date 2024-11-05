use std::collections::HashMap;

use super::Schema;
use super::Tool as ToolMetadata;

pub type ToolCollection = Vec<Box<dyn Tool>>;

pub trait Tool
where
  Self: Send + Sync,
{
  fn metadata(&self) -> &ToolMetadata;
  fn invoke(&mut self, params: serde_json::Value) -> Result<Option<String>, String>;
}

pub fn invoke_tool(
  collection: &mut ToolCollection,
  name: &str,
  input: serde_json::Value,
) -> Result<Option<String>, String> {
  let tool = collection
    .iter_mut()
    .find(|tool| tool.metadata().name == name)
    .ok_or_else(|| "No tool found!".to_string())?;

  tool.invoke(input)
}

pub struct FetchTool(ToolMetadata);

impl FetchTool {
  pub fn new() -> Self {
    let mut properties = HashMap::new();
    properties.insert(
      "url".into(),
      Schema::String {
        description: "the full path to a website to retrieve, starting with http:// or https://"
          .into(),
      },
    );

    let input_schema = Schema::Object {
      properties,
      required: vec!["url".into()],
    };

    Self(ToolMetadata {
      name: "fetch_url".into(),
      description: "Retrieve the textual representation of a given webpage".into(),
      input_schema,
    })
  }
}

impl Tool for FetchTool {
  fn metadata(&self) -> &ToolMetadata {
    &self.0
  }

  fn invoke(&mut self, params: serde_json::Value) -> Result<Option<String>, String> {
    let url = params
      .as_object()
      .and_then(|obj| obj.get("url"))
      .and_then(|s| s.as_str())
      .ok_or("No URL provided!".to_string())?;
    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let body = resp.into_string().map_err(|e| e.to_string())?;

    let doc = scraper::Html::parse_document(&body);
    let text = doc.root_element().text().collect::<Vec<_>>();

    Ok(Some(text.join("")))
  }
}
