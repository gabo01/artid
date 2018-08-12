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
use env_path::EnvPath;
use failure::ResultExt;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

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

pub struct ConfigFile<P>
where
    P: AsRef<Path>,
{
    path: P,
    folders: Vec<Folder>,
}

impl<P> ConfigFile<P>
where
    P: AsRef<Path>,
{
    const RESTORE: &'static str = ".backup/config.json";

    pub fn load(path: P) -> Result<Self> {
        let file = Self::filepath(&path)?;

        let reader =
            File::open(&file).context(AppErrorType::AccessFile(file.display().to_string()))?;
        let folders = json::from_reader(reader)
            .context(AppErrorType::JsonParse(file.display().to_string()))?;
        trace!("{:?}", folders);

        Ok(ConfigFile { path, folders })
    }

    pub fn save<T: AsRef<Path>>(&self, location: T) -> Result<()> {
        let path = location.as_ref();
        write!(
            File::create(&path).context(AppErrorType::AccessFile(path.display().to_string()))?,
            "{}",
            json::to_string_pretty(self.folders).expect("ConfigFile cannot fail serialization")
        ).context(AppErrorType::AccessFile(path.display().to_string()))?;

        info!("Config file saved on {}", pathlight(self.path.as_ref()));
        Ok(())
    }

    pub fn save_default(&self) -> Result<()> {
        self.save(Self::filepath(&self.path)?)
    }

    pub fn backup<T: AsRef<Path>>(mut self, root: T) -> Result<()> {
        let mut error = None;
        for folder in &mut self.folders {
            let dirs = folder.resolve(&root);
            debug!("Starting backup of: {}", pathlight(&dirs.abs));

            if let Err(err) =
                LinkTree::new(dirs.rel, dirs.abs)
                    .sync()
                    .context(AppErrorType::UpdateFolder(
                        root.as_ref().display().to_string(),
                    )) {
                error = Some(err);
                break;
            }

            folder.modified = Some(Utc::now());
        }

        if let Err(err) = self.save_default() {
            warn!(
                "Unable to save on {} because of {}",
                pathlight(self.path.as_ref()),
                err
            );
        }

        match error {
            Some(err) => err!(err.into()),
            None => Ok(()),
        }
    }

    pub fn restore<T: AsRef<Path>>(self, root: T) -> Result<()> {
        for folder in &self.folders {
            let dirs = folder.resolve(&root);
            debug!("Starting restore of: {}", pathlight(&dirs.rel));

            LinkTree::new(dirs.abs, dirs.rel)
                .sync()
                .context(AppErrorType::RestoreFolder(
                    root.as_ref().display().to_string(),
                ))?;
        }

        Ok(())
    }

    fn filepath<T: AsRef<Path>>(path: T) -> Result<PathBuf> {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Folder {
    path: EnvPath,
    origin: EnvPath,
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
