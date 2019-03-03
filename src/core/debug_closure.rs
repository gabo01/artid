use std::fmt::{self, Debug};
use std::ops::Deref;

/// Allow a debug implementation for closures.
///
/// details of the implementation can be found in [location][link]
///
/// [link]: https://users.rust-lang.org/t/is-it-possible-to-implement-debug-for-fn-type/14824/3
pub(crate) struct Debuggable<T: ?Sized> {
    pub(crate) text: &'static str,
    pub(crate) value: Box<T>,
}

impl<T: ?Sized> Debug for Debuggable<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl<T: ?Sized> Deref for Debuggable<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}
