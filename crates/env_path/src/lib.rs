extern crate regex;
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "serde")]
extern crate serde;

use regex::{Captures, Regex};
#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};
use std::env;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

/// Represents an enviroment path. An enviroment path is represented as a standard string
/// that may have interpolated enviroment variables, refered to as addr, and a path that is
/// pointed by the addr.
#[derive(Debug, PartialEq)]
pub struct EnvPath {
    addr: String,
    path: PathBuf,
}

impl EnvPath {
    /// Creates a new EnvPath from a given addr. Notice that the translation of the addr
    /// into the path will occur in these step. This means that setting the enviroment variable
    /// later will have no effect on the registered path.
    pub fn new<T: Into<String>>(addr: T) -> Self {
        let addr = addr.into();
        let path = PathBuf::from(Self::regex(&addr));

        Self { addr, path }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn regex(path: &str) -> String {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(\\?)\$([A-Z]+)").unwrap();
        }

        RE.replace_all(path, |x: &Captures| {
            if &x[1] == r"\" {
                return x[0].replace(r"\", "");
            }

            match env::var(&x[2]) {
                Ok(s) => s,
                Err(_) => String::from(""),
            }
        }).to_string()
    }
}

#[cfg(feature = "serde")]
impl<'de> Serialize for EnvPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.addr.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for EnvPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let addr = <String as Deserialize<'de>>::deserialize(deserializer)?;
        Ok(Self::new(addr))
    }
}

impl AsRef<Path> for EnvPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Display for EnvPath {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::EnvPath;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn interpolate_vars() {
        let home = env::var("HOME").unwrap();
        let env_path = EnvPath::new("$HOME");
        assert_eq!(home, env_path.path().display().to_string());
    }

    #[test]
    fn interpolate_join() {
        let home = env::var("HOME").unwrap();
        let env_path = EnvPath::new("$HOME/Templates");
        assert_eq!(
            PathBuf::from(home).join("Templates").display().to_string(),
            env_path.path().display().to_string()
        );
    }

    #[test]
    fn display() {
        let path = EnvPath::new("/home/gabo01");
        assert_eq!(path.to_string(), path.path.display().to_string());
    }
}
