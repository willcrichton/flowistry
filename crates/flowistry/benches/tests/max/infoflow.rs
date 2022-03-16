extern crate bench_utils;
use bench_utils::generate_flow;

pub fn main() {
  generate_flow!(_x: [i32; 250] = 1);
}
