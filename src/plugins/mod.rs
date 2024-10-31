use crate::claude::Tool;
use anyhow::anyhow;
use extism::{convert::Json, Manifest, Plugin, PluginBuilder, Wasm};
use log::{error, info};

#[derive(Debug)]
pub struct Host {
  path: String,
  pub plugins: Vec<(Plugin, Vec<Tool>)>,
}

impl Host {
  pub fn invoke_tool(
    &mut self,
    name: &str,
    input: serde_json::Value,
  ) -> anyhow::Result<Option<String>> {
    if let Some((p, _t)) = self
      .plugins
      .iter_mut()
      .find(|(_, tools)| tools.iter().any(|t| t.name == name))
    {
      match p.call::<&str, &str>(name, &serde_json::to_string(&input).unwrap()) {
        Ok(s) => Ok(Some(s.to_owned())),
        Err(e) => Err(e),
      }
    } else {
      Err(anyhow!("No matching tool found"))
    }
  }

  pub fn new<S: Into<String> + AsRef<str>>(path: S) -> Self {
    Self {
      plugins: vec![],
      path: path.into(),
    }
  }

  pub fn load(&mut self) -> Result<(), anyhow::Error> {
    self.plugins.clear();

    for entry in std::fs::read_dir(&self.path)? {
      let entry = entry?;
      let path = entry.path();
      if let Some("wasm") = path.extension().and_then(|os| os.to_str()) {
        info!("Loading plugin from {:?}", path);

        let wasm = Wasm::file(&path);
        let manifest = Manifest::new([wasm]);
        let mut plugin = PluginBuilder::new(manifest)
          .with_wasi(true)
          .build()
          .unwrap();
        match plugin.call::<(), Json<Vec<Tool>>>("getTools", ()) {
          Ok(Json(tools)) => {
            info!("Loaded!");
            self.plugins.push((plugin, tools));
          }
          Err(e) => error!("Error loading {:?}: {}", path, e),
        };
      }
    }

    Ok(())
  }
}
