fn main() {
  env_logger::init();
  rustc_plugin::cli_main(flowistry_ifc::IfcPlugin);
}
