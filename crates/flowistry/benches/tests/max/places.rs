extern crate bench_utils;
use bench_utils::generate_places;

pub fn main() {
  generate_places!(_x: PlaceStruct<[i32; 1000]> = 1);
}
