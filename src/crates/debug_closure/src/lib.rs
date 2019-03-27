use std::fmt::{self, Debug};
use std::ops::Deref;

#[macro_export]
macro_rules! closure {
    ($($body:tt)+) => {
        Debuggable::new(stringify!($($body)+), Box::new($($body)+))
    };
}

/// Allow a debug implementation for closures.
///
/// details of the implementation can be found in [location][link]
///
/// [link]: https://users.rust-lang.org/t/is-it-possible-to-implement-debug-for-fn-type/14824/3
pub struct Debuggable<T: ?Sized> {
    text: &'static str,
    value: Box<T>,
}

impl<T: ?Sized> Debuggable<T> {
    pub fn new(text: &'static str, value: Box<T>) -> Self {
        Self { text, value }
    }

    pub fn into_box(self) -> Box<T> {
        self.value
    }
}

impl<T: ?Sized> Debug for Debuggable<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl<T: ?Sized> Deref for Debuggable<T> {
    type Target = Box<T>;

    fn deref(&self) -> &Box<T> {
        &self.value
    }
}
