#![allow(deprecated)]

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
extern crate yansi;

extern crate env_path;

use chrono::offset::Utc;
use chrono::DateTime;
use failure::ResultExt;

use std::fs::File;
use std::path::{Path, PathBuf};

use env_path::EnvPath;

pub mod errors;
mod fs;
pub mod logger;

pub use errors::{AppError, AppErrorType};
use fs::LinkTree;
use logger::pathlight;

macro_rules! err {
    ($x:expr) => {
        return Err($x);
    };
}

pub type Result<T> = ::std::result::Result<T, AppError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    folders: Vec<Folder>,
}

impl ConfigFile {
    const RESTORE: &'static str = ".backup/config.json";

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = Self::filepath(path)?;

        let reader =
            File::open(&file).context(AppErrorType::AccessFile(file.display().to_string()))?;
        let folders = json::from_reader(reader)
            .context(AppErrorType::JsonParse(file.display().to_string()))?;
        trace!("{:?}", folders);

        Ok(ConfigFile { folders })
    }

    pub fn backup<P: AsRef<Path>>(self, root: P) -> Result<()> {
        for folder in self {
            let dirs = folder.resolve(&root);
            debug!("Starting backup of: {}", pathlight(&dirs.abs));

            let mut tree = LinkTree::new(dirs.rel, dirs.abs);
            fs::backup(&mut tree).context(AppErrorType::UpdateFolder(
                root.as_ref().display().to_string(),
            ))?
        }

        Ok(())
    }

    pub fn restore<P: AsRef<Path>>(self, root: P) -> Result<()> {
        for folder in self {
            let dirs = folder.resolve(&root);
            debug!("Starting restore of: {}", pathlight(&dirs.rel));

            let mut tree = LinkTree::new(dirs.abs, dirs.rel);
            fs::backup(&mut tree).context(AppErrorType::RestoreFolder(
                root.as_ref().display().to_string(),
            ))?
        }

        Ok(())
    }

    fn filepath<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
        let path = path.as_ref();

        if !path.is_dir() {
            err!(AppError::from(AppErrorType::NotDir(
                path.display().to_string()
            )));
        }

        let restore = path.join(Self::RESTORE);
        if !restore.is_file() {
            err!(AppError::from(AppErrorType::PathUnexistant(
                restore.display().to_string()
            )));
        }

        debug!("config file: {}", pathlight(path));
        Ok(restore)
    }
}

impl IntoIterator for ConfigFile {
    type Item = Folder;
    type IntoIter = std::vec::IntoIter<Folder>;

    fn into_iter(self) -> Self::IntoIter {
        self.folders.into_iter()
    }
}

impl<'a> IntoIterator for &'a ConfigFile {
    type Item = &'a Folder;
    type IntoIter = std::slice::Iter<'a, Folder>;

    fn into_iter(self) -> Self::IntoIter {
        self.folders.iter()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Folder {
    path: EnvPath,
    origin: EnvPath,
    description: String,
    modified: Option<DateTime<Utc>>, // parses from an RFC3339 valid string
}

struct Dirs {
    rel: PathBuf,
    abs: PathBuf,
}

impl Folder {
    fn resolve<P: AsRef<Path>>(&self, root: P) -> Dirs {
        Dirs {
            rel: root.as_ref().join(self.path.as_ref()),
            abs: PathBuf::from(self.origin.as_ref()),
        }
    }
}
