extern crate chrono;
extern crate failure;
#[macro_use]
extern crate failure_derive;

use chrono::DateTime;
use chrono::offset::Utc;
use std::path::{Path, PathBuf};

pub mod actions;
pub mod errors;
pub mod logger;

pub use errors::{AppError, AppErrorType};

pub type Result<T> = ::std::result::Result<T, AppError>;

pub struct ConfigFile {
    folders: Vec<Folder>
}

impl ConfigFile {
    const RESTORE: &'static str = ".backup/restore.json";

    pub fn load<T: AsRef<Path>>(path: T) -> Result<Self> {
        let _file = Self::filepath(path)?;
        Ok(ConfigFile {
            folders: vec![]
        })
    }

    fn filepath<T: AsRef<Path>>(path: T) -> Result<PathBuf> {
        let path = path.as_ref();

        if !path.is_dir() {
            return Err(AppError::from(AppErrorType::NotDir(path.display().to_string())));
        }

        let restore = path.join(Self::RESTORE);
        if !restore.is_file() {
            return Err(AppError::from(AppErrorType::PathUnexistant(restore.display().to_string())));
        }

        Ok(restore)
    }
}

pub struct Folder {
    path: String,
    origin: String,
    description: String,
    modified: Option<DateTime<Utc>> // parses from an RFC3339 valid string
}
