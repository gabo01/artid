use std::fmt::{self, Display};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathRepr {
    inner: String,
}

impl<P: AsRef<Path>> From<P> for PathRepr {
    fn from(path: P) -> Self {
        Self {
            inner: path.as_ref().display().to_string(),
        }
    }
}

impl Display for PathRepr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}
