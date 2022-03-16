extern crate bench_utils;
use bench_utils::generate_flow;

pub fn main() {
  // This macro generates twice as many locations as generate_unique_lifetimes
  // and generate_locations because each iteration creates a place and then reassigns
  // the "main" variable using that place as an input (two distinct steps). To exert 
  // equivalent location stress as done in lifetimes_unique.rs and locations.rs, we 
  // divide the number of iterations by two (200 / 2).
  generate_flow!(_x: [i32; 100] = 1);
}
