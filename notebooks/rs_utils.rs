// serde_json = "1.0"
// serde = {version = "1.0", features = ["derive"]}
// pythonize = "0.13"
// rayon = "1.5"

use pyo3::prelude::*;
use pyo3::{exceptions::PyException, wrap_pyfunction};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

#[derive(Serialize, Deserialize, Clone)]
pub struct Range {
  pub start_line: usize,
  pub start_col: usize,
  pub end_line: usize,
  pub end_col: usize,
  pub filename: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum MutabilityMode {
  DistinguishMut,
  IgnoreMut,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ContextMode {
  SigOnly,
  Recurse,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum PointerMode {
  Precise,
  Conservative,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EvalResult {
  mutability_mode: MutabilityMode,
  context_mode: ContextMode,
  pointer_mode: PointerMode,
  sliced_local: usize,
  function_range: Range,
  function_path: String,
  num_instructions: usize,
  num_relevant_instructions: usize,
  num_tokens: usize,
  num_relevant_tokens: usize,
  duration: f64,
  has_immut_ptr_in_call: bool,
  has_same_type_ptrs_in_call: bool,
  has_same_type_ptrs_in_input: bool,
  reached_library: bool,
  // added fields
  instructions_relative: Option<usize>,
  instructions_relative_frac: Option<f64>,
  instructions_relative_base: Option<usize>,
  instructions_relative_base_frac: Option<f64>,
  baseline_reached_library: Option<bool>,
}

#[pyfunction]
fn parse_data(py: Python, path: String) -> PyResult<PyObject> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);
  let mut data: Vec<Vec<EvalResult>> =
    serde_json::from_reader(reader).map_err(|err| PyException::new_err(format!("{}", err)))?;

  let updated_data = data
    .par_iter_mut()
    .map(|trial| {
      let min_sample = trial
        .iter()
        .min_by_key(|sample| sample.num_relevant_instructions)
        .cloned()
        .unwrap();
      let base_sample = trial
        .iter()
        .find(|sample| {
          sample.mutability_mode == MutabilityMode::DistinguishMut
            && sample.context_mode == ContextMode::SigOnly
            && sample.pointer_mode == PointerMode::Precise
        })
        .cloned()
        .unwrap();
      trial
        .into_iter()
        .map(|mut sample| {
          let min_inst = min_sample.num_relevant_instructions;
          sample.instructions_relative = Some(sample.num_relevant_instructions - min_inst);
          sample.instructions_relative_frac =
            Some((sample.num_relevant_instructions - min_inst) as f64 / (min_inst as f64));
          sample.reached_library = min_sample.reached_library;
          let base_inst = base_sample.num_relevant_instructions;
          sample.instructions_relative_base = Some(sample.num_relevant_instructions - base_inst);
          sample.instructions_relative_base_frac =
            Some((sample.num_relevant_instructions - base_inst) as f64 / (base_inst as f64));
          sample.baseline_reached_library = Some(min_sample.reached_library);
          sample
        })
        .collect::<Vec<_>>()
    })
    .flatten()
    .collect::<Vec<_>>();

  Ok(pythonize::pythonize(py, &updated_data)?)
}

#[pymodule]
fn rs_utils(py: Python, m: &PyModule) -> PyResult<()> {
  m.add_function(wrap_pyfunction!(parse_data, m)?)?;
  Ok(())
}
