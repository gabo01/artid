#![allow(deprecated)]
#![warn(missing_docs)]
#![allow(clippy::new_ret_no_self)]
#![allow(unused_imports)]

//! This is the core of artid. Artid is intended to be a backup application with focus
//! on versioning and maintaining multiple backups of a single location. to know more
//! about artid see [README.md][read].
//!
//! [read]: https://github.com/gabo01/artid/blob/master/README.md
//!
//! ## About the core
//!
//! The core of artid contains all the logic for the operations performed. It's responsability
//! is to provide three things:
//!
//! - an API to perform common operations: backup, restore, zip (TODO) and others
//! - an API to generate custom operations: this is made through the ops::core module but needs
//!   to be polished and as such is not to be relied on
//! - an API to access state of the backup (not implemented yet)
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
//! - ops::core: the core module provides the bare bone elements for generating the operations
//!              and will be the API exported to generate custom operations. An example of
//!              what the core module does is defining the data type responsible for comparing
//!              two folders.
//!
//! ## Integration with frontends
//!
//! The core as a library is not suited for integration with a non-rust written frontend,
//! in order to integrate this library with an external frontend, for example electron, a
//! bridge must be written.

macro_rules! rfc3339 {
    ($stamp:expr) => {{
        use chrono::SecondsFormat;
        $stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)
    }};
}

macro_rules! closure {
    ($($body:tt)+) => {
        Debuggable {
            text: stringify!($($body)+),
            value: Box::new($($body)+),
        }
    };
}

#[cfg(test)]
#[macro_use]
mod tools;

mod debug_closure;
mod fn_box;

pub mod config;
pub mod ops;

#[allow(missing_docs)]
pub mod prelude {
    pub use crate::config::{ConfigFile, FileError, FileSystemFolder, FolderConfig};
    pub use crate::ops::{Model, Operation, Operator};
}
