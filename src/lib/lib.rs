#![allow(deprecated)]

extern crate atty;
extern crate chrono;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate libc;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
#[cfg(test)]
extern crate tempfile;
extern crate yansi;

extern crate env_path;

macro_rules! rfc3339 {
    ($stamp:expr) => {{
        use chrono::SecondsFormat;
        $stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)
    }};
}

#[cfg(test)]
#[macro_use]
mod tools;

mod config;
pub mod logger;
mod ops;
mod sync;

pub mod prelude {
    pub use config::{ConfigFile, Folder};
    pub use ops::{BackupOptions, RestoreOptions};
}

pub mod errors {
    pub use config::{LoadError, SaveError};
    pub use ops::{BackupError, RestoreError};
}
