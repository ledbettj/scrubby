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
    .find(|tool| tool.metadata().name() == name)
    .ok_or_else(|| "No tool found!".to_string())?;

  tool.invoke(input)
}

pub struct FetchTool(ToolMetadata);

impl FetchTool {
  pub fn new() -> Self {
    Self(ToolMetadata::Custom {
      name: "fetch_url".into(),
      description: "Retrieve the textual representation of a given webpage.  This tool should only be used when you are explicitly asked to fetch a webpage.".into(),
      input_schema: Schema::object().with_property(
        "url",
        Schema::string("the full path to a website to retrieve, starting with http:// or https://"),
        true,
      ),
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
    let mut text = doc.root_element().text().collect::<Vec<_>>().join("");
    if text.len() > 1200 {
      text = text[..1024].to_string();
      text += " <truncated>";
    }
    Ok(Some(text))
  }
}
