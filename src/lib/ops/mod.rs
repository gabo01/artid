//! Contains the logic for all the common operations performed by artid.
//! 
//! This module serves as a way to represent the common operations based on the information
//! given by the sync module.

mod backup;
mod errors;
mod restore;

pub use self::{
    backup::{Backup, BackupOptions},
    errors::{BackupError, BackupErrorType, RestoreError, RestoreErrorType},
    restore::{Restore, RestoreOptions},
};
