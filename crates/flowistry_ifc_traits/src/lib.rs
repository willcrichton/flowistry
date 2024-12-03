use std::fmt;

pub trait Secure {}
pub trait Insecure {}

impl<T: Secure> Secure for &T {}

impl Insecure for fmt::Arguments<'_> {}

pub struct InsecureString(pub String);
impl Insecure for InsecureString {}

#[macro_export]
macro_rules! insecure_print {
  ($($arg:tt),*) => {
    let s = $crate::InsecureString(format!($($arg),+));
    println!("{}", s.0);
  }
}
