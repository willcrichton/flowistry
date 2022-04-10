extern crate bench_utils;
use bench_utils::generate_locations;

pub fn main() {
  generate_locations!(_x: [i32; 200] = 1);
}
