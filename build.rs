use std::{
  env,
  error::Error,
  fs::{self, File},
  io::Write,
  path::Path,
};

fn load_files(path: &str, root_path: &str, packages: &mut File) -> Result<(), Box<dyn Error>> {
  for f in fs::read_dir(path)? {
    let f = f?;
    let ft = f.file_type()?;

    if ft.is_dir() {
      load_files(&f.path().to_string_lossy(), root_path, packages)?;
    } else if ft.is_file() {
      let p = f.path();
      if p.extension().and_then(|s| s.to_str()) != Some("lua") {
        continue;
      }
      writeln!(
        packages,
        r##"("{rel_name}", include_str!(r#"{name}"#)),"##,
        rel_name = p.strip_prefix(root_path)?.display(),
        name = p.display()
      )?;
    } else {
      continue;
    }
  }

  Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
  let source_dir: &str = &format!("{}/pkgs/", env::var("CARGO_MANIFEST_DIR")?);
  let out_dir = env::var("OUT_DIR")?;
  let dest_path = Path::new(&out_dir).join("lua_packages.rs");
  let mut packages = File::create(&dest_path)?;

  writeln!(&mut packages, r##"["##,)?;
  load_files(source_dir, source_dir, &mut packages)?;
  writeln!(&mut packages, r##"]"##,)?;
  Ok(())
}
