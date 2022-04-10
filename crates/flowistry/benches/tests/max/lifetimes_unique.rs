extern crate bench_utils;
use bench_utils::generate_unique_lifetimes;

pub fn main() {
  generate_unique_lifetimes!(_x: [i32; 1000] = 1);
}
