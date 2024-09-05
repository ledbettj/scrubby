use std::collections::HashMap;

use log::info;
use rhai::{
  module_resolvers::{FileModuleResolver, ModuleResolversCollection, StaticModuleResolver},
  CustomType, Dynamic, Engine, FnPtr, FuncRegistration, Module, Position, TypeBuilder, AST,
};

use crate::claude::Schema;

#[derive(Debug)]
pub struct Host {
  path: String,
  engine: Engine,
  pub plugins: Vec<Plugin>,
  pub tools: Vec<Tool>,
}

impl Host {
  pub fn invoke_tool(
    &self,
    name: &str,
    input: HashMap<String, String>,
  ) -> anyhow::Result<Option<String>> {
    let tool = self
      .tools
      .iter()
      .find(|t| t.inner.name == name)
      .ok_or(anyhow::Error::msg("No tool found"))?;

    let input: rhai::Map = input
      .into_iter()
      .map(|(k, v)| (k.into(), v.into()))
      .collect();
    let s: String = tool.func.call(&self.engine, &AST::empty(), (input,))?;
    Ok(Some(s))
  }

  pub fn eval<S: AsRef<str>>(&self, input: S) -> String {
    match self.engine.eval::<Dynamic>(input.as_ref()) {
      Err(e) => e.to_string(),
      Ok(d) => format!("{}", d).to_string(),
    }
  }

  pub fn new<S: Into<String> + AsRef<str>>(path: S) -> Self {
    let mut engine = Engine::new();
    let mut bot = Module::new();

    FuncRegistration::new("plugin").set_into_module(&mut bot, |name: &str| Plugin {
      name: name.into(),
      tools: HashMap::new(),
    });

    let mut static_resolver = StaticModuleResolver::new();
    static_resolver.insert("bot", bot);

    let file_resolver = FileModuleResolver::new_with_path(path.as_ref());
    let mut resolver = ModuleResolversCollection::new();

    resolver.push(static_resolver);
    resolver.push(file_resolver);

    engine.build_type::<Plugin>();
    engine.register_fn("tool", Plugin::tool);
    engine.register_type_with_name::<Schema>("Schema");
    engine.set_module_resolver(resolver);

    engine.set_max_strings_interned(1024);

    engine.on_progress(|ops| {
      if ops > 10_000 {
        Some("Evaluation killed due to timeout.".into())
      } else {
        None
      }
    });

    Self {
      engine,
      plugins: vec![],
      path: path.into(),
      tools: vec![],
    }
  }

  pub fn load(&mut self) -> Result<(), anyhow::Error> {
    let resolver = self.engine.module_resolver();

    for entry in std::fs::read_dir(&self.path)? {
      let entry = entry?;
      let path = entry.path();
      if let Some("rhai") = path.extension().and_then(|os| os.to_str()) {
        if let Some(file_name) = path.file_stem().and_then(|os| os.to_str()) {
          info!("importing {}", file_name);
          let plug = resolver
            .resolve(&self.engine, None, file_name, Position::NONE)
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;

          let plugins: Vec<Plugin> = plug
            .iter_var()
            .filter_map(|(_, obj)| obj.clone().try_cast::<Plugin>())
            .collect();

          self.plugins.extend(plugins);
        }
      }
    }

    self.tools = self
      .plugins
      .iter()
      .flat_map(|plugin| plugin.tools.values().cloned())
      .collect::<Vec<Tool>>();

    Ok(())
  }
}

#[derive(Debug, Clone, CustomType)]
pub struct Plugin {
  pub name: String,
  pub tools: HashMap<String, Tool>,
}

#[derive(Debug, Clone, CustomType)]
pub struct Tool {
  pub func: FnPtr,
  pub inner: crate::claude::Tool,
}

impl Plugin {
  fn tool(&mut self, name: &str, description: &str, schema: Dynamic, callback: FnPtr) {
    let input_schema = rhai::serde::from_dynamic::<Schema>(&schema).unwrap();
    let t = Tool {
      inner: crate::claude::Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
      },
      func: callback,
    };
    self.tools.insert(name.into(), t);
  }
}
