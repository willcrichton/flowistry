extern crate bench_utils;
use bench_utils::generate_same_lifetime;

pub fn main() {
  generate_same_lifetime!(_x: LifetimesStruct<[i32; 1000]> = 1);
}
