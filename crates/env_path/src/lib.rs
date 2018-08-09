extern crate regex;
#[macro_use]
extern crate lazy_static;

use regex::{Captures, Regex};
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
