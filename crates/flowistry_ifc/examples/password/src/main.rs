use flowistry_ifc_traits::{insecure_print, Secure};
struct Password(&'static str);
impl Secure for Password {}

fn main() {
  let password = Password(include_str!("secret"));
  if password.0 == "hello" {
    insecure_print!("Hello world!");
  }
}

