use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize, Debug)]
struct Target {
  name: String,
  crate_types: Vec<String>,
  edition: String,
  src_path: String,
}

#[derive(Deserialize, Debug)]
struct Dependency {
  index: usize,
}

#[derive(Deserialize, Debug)]
struct Unit {
  target: Target,
  dependencies: Vec<Dependency>,
  features: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct UnitGraph {
  units: Vec<Unit>,
  roots: Vec<usize>,
}

impl UnitGraph {
  fn run_cargo_and_build() -> Result<Self> {
    let cargo_output = Command::new("cargo")
      .args(&["check", "--unit-graph", "-Z", "unstable-options"])
      .output()?
      .stdout;
    Ok(serde_json::from_slice::<UnitGraph>(&cargo_output)?)
  }

  fn find_unit_containing(&self, target_path: &Path) -> Option<&Unit> {
    self.units.iter().find(|unit| {
      let src_path = Path::new(&unit.target.src_path);
      match src_path.parent() {
        Some(src_dir) => target_path.ancestors().any(|ancestor| ancestor == src_dir),
        None => false,
      }
    })
  }
}

fn gather_rmeta_paths() -> Result<HashMap<String, String>> {
  let re = Regex::new(r"lib(.+)-[\w\d]+.rmeta")?;
  Ok(
    fs::read_dir("target/debug/deps")?
      .map(|file| {
        Ok({
          let path = file?.path();
          let path_str = path.to_str().context("Couldn't convert path")?;
          if let Some(file_name) = path.file_name() {
            let file_name = file_name.to_str().context("Couldn't convert file name")?;
            re.captures(file_name).map(|capture| {
              (
                capture.get(1).unwrap().as_str().to_owned(),
                path_str.to_owned(),
              )
            })
          } else {
            None
          }
        })
      })
      .collect::<Result<Vec<_>>>()?
      .into_iter()
      .filter_map(|x| x)
      .collect::<HashMap<_, _>>(),
  )
}

pub fn get_flags(target_path: &str) -> Result<Vec<String>> {
  let target_path = Path::new(target_path);

  let graph = UnitGraph::run_cargo_and_build()?;

  let target_unit = graph.find_unit_containing(&target_path).context(format!(
    "Could not find unit with source directory for {}",
    target_path.display()
  ))?;

  // Run cargo check to generate dependency rmetas
  {
    let check = Command::new("cargo")
      .args(&["check", "--package", &target_unit.target.name])
      .output()?;
    if !check.status.success() {
      bail!(
        "cargo check failed with error: {}",
        String::from_utf8(check.stderr)?
      );
    }
  }

  let rmeta_paths = gather_rmeta_paths()?;

  #[rustfmt::skip]
  let unit_flags = vec![
    "rustc".into(),    
    
    "--crate-name".into(), target_unit.target.name.clone(),

    // TODO: what if there are multiple crate types?
    "--crate-type".into(), target_unit.target.crate_types[0].clone(),

    // Path must be the crate root file, NOT the sliced file
    target_unit.target.src_path.clone(),

    format!("--edition={}", target_unit.target.edition),

    "-L".into(), "dependency=target/debug/deps".into(),

    // Avoids ICE looking for MIR data?
    "--emit=dep-info,metadata".into(),
  ];

  let feature_flags = target_unit
    .features
    .iter()
    .map(|feature| vec!["--cfg".into(), format!("feature=\"{}\"", feature)])
    .flatten();

  let extern_flags = target_unit
    .dependencies
    .iter()
    .map(|dep| {
      let dep_unit = &graph.units[dep.index];

      // packages like `percent-encoding` are translated to `percent_encoding`
      let package_name = dep_unit.target.name.replace("-", "_");

      let rmeta_path = &rmeta_paths
        .get(&package_name)
        .expect(&format!("Missing rmeta for `{}`", package_name));

      vec![
        "--extern".into(),
        format!("{}={}", package_name, rmeta_path),
      ]
    })
    .flatten();
  Ok(
    unit_flags
      .into_iter()
      .chain(feature_flags)
      .chain(extern_flags)
      .collect(),
  )
}
