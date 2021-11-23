pub trait Secure {}
pub trait Insecure {}

impl<T: Secure> Secure for &T {}

pub struct InsecureString(pub String);
impl Insecure for InsecureString {}

#[macro_export]
macro_rules! insecure_print {
  ($($arg:tt),*) => {
    let s = $crate::InsecureString(format!($($arg),+));
    println!("{}", s.0);
  }
}
