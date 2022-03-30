extern crate bench_utils;
use bench_utils::generate_nested_struct;

pub fn main() {
    generate_nested_struct!(_x: NestedStruct<[i32; 5]> = 1);
}
