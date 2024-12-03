#![feature(rustc_private)]

fn main() {
  env_logger::init();
  rustc_plugin::driver_main(flowistry_ifc::IfcPlugin);
}
