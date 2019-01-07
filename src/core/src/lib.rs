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

extern crate artid_logger as logger;

extern crate chrono;
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
#[cfg(test)]
extern crate tempfile;

extern crate env_path;

use std::fmt::{self, Debug};
use std::ops::Deref;

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

pub mod config;
mod sync;

/// Allows to call boxed closures
///
/// details about this helper can be found on the rust book chapter 20: building a
/// multi-threaded web server
trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

/// Allow a debug implementation for closures.
///
/// details of the implementation can be found in [location][link]
///
/// [link]: https://users.rust-lang.org/t/is-it-possible-to-implement-debug-for-fn-type/14824/3
struct Debuggable<T: ?Sized> {
    text: &'static str,
    value: Box<T>,
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

#[allow(missing_docs)]
pub mod prelude {
    pub use config::{BackupOptions, ConfigFile, FileSystemFolder, FolderConfig, RestoreOptions};
}

#[allow(missing_docs)]
pub mod errors {
    pub use config::{FileError, OperativeError, OperativeErrorType};
}
