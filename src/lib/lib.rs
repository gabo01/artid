#![allow(deprecated)]
#![warn(missing_docs)]

//! This is the core of artid. Artid is intended to be a backup application with focus
//! on versioning and maintaining multiple backups of a single location. to know more
//! about artid see [README.md][read].
//! 
//! [read]: https://github.com/gabo01/artid/blob/master/README.md
//! 
//! ## About the core
//! 
//! The core of artid contains all the logic for the operations performed. Currently, it
//! also contains some display related procedures (like CopyModel::log) or the logger module
//! that are on the way of being refactored away to avoid displaying the data themselves.
//! Leaving aside the details that need to be polished, the core's responsability is to 
//! provide two things:
//! 
//! - an API to perform common operations: backup, restore, zip (TODO) and others
//! - an API to generate custom operations: this is made through the sync module
//!   but it's not currently part of the public API.
//! 
//! ## About the structure
//! 
//! The core is divided into several parts based on the functionality they provide. Here
//! is a general list of the different parts and what they do:
//! 
//! - config: the config module is responsible of managing everything related to the artid
//!           configuration file config.json found usually on .backup/config.json. It also 
//!           calls the common operations on the config file elements
//! - ops: the ops module is responsible of performing the common operation shipped with
//!        artid
//! - sync: the sync module provides the bare bone elements for generating the operations 
//!         and will be the API exported to generate custom operations. An example of 
//!         what the sync module does is defining the data type responsible for comparing 
//!         two folders.
//! 
//! ## Integration with frontends
//! 
//! The core as a library is not suited for integration with a non-rust written frontend,
//! in order to integrate this library with an external frontend, for example electron, a
//! bridge must be written.

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

/// The prelude contains the most commonly used structures of artid and as such represents
/// an easy way to access to them.
pub mod prelude {
    pub use config::{ConfigFile, Folder};
    pub use ops::{BackupOptions, RestoreOptions};
}

/// Contains the errors that can be thrown by the application components.
pub mod errors {
    pub use config::FileError;
    pub use ops::OperativeError;
}
