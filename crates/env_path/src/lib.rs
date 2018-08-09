extern crate regex;
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "serde")]
extern crate serde;

use regex::{Captures, Regex};
#[cfg(feature = "serde")]
use serde::{
    ser::{Serialize, Serializer},
    de::{Deserialize, Deserializer}
};
use std::env;
use std::path::{Path, PathBuf};

pub struct EnvPath {
    addr: String,
    path: PathBuf
}

impl EnvPath {
    pub fn new<T: Into<String>>(addr: T) -> Self {
        let addr = addr.into();
        let path = PathBuf::from(Self::regex(&addr));

        Self {
            addr,
            path
        }
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
                Err(_) => String::from("")
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
        let addr = <String ad Deserialize<'de>>::deserialize(deserializer)?;
        Ok(Self::new(addr))
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;
    use super::EnvPath;

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
        assert_eq!(PathBuf::from(home).join("Templates").display().to_string(), env_path.path().display().to_string());
    }
}
