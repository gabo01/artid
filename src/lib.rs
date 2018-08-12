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

/// Alias for the Result type
pub type Result<T> = ::std::result::Result<T, AppError>;

/// Represents a configuration file stored in config.json. A valid config.json file is composed
/// by an array of folder structs. Usually this file will be stored in the directory intended
/// to be the backup directory. Even if it is technically possible to use single configuration
/// file to manage multiple backup dirs, this is not wise since the modified date for the folders
/// inside the config file will lose it's proper meaning.
/// 
/// This type is also the main point of entry for the library since it controls the 
/// loading of the configuration and allows to do the ops related to the data inside, such 
/// as the backup and restore of the files.
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

    /// Loads the data present in the configuration file. Currently this function receives
    /// a directory and looks for the config file in the subpath .backup/config.json.
    pub fn load(path: P) -> Result<Self> {
        let file = Self::filepath(&path)?;

        let reader =
            File::open(&file).context(AppErrorType::AccessFile(file.display().to_string()))?;
        let folders = json::from_reader(reader)
            .context(AppErrorType::JsonParse(file.display().to_string()))?;
        trace!("{:?}", folders);

        Ok(ConfigFile { path, folders })
    }

    /// Saves the changes made back to the config.json file. Currently used more in a private
    /// fashion to update the last date when the folders were synced.
    /// 
    /// Same as the load function. This function receives a directory and looks for the
    /// config.json file inside .backup/config.json.
    pub fn save<T: AsRef<Path>>(&self, location: T) -> Result<()> {
        let path = location.as_ref();
        write!(
            File::create(&path).context(AppErrorType::AccessFile(path.display().to_string()))?,
            "{}",
            json::to_string_pretty(&self.folders).expect("ConfigFile cannot fail serialization")
        ).context(AppErrorType::AccessFile(path.display().to_string()))?;

        info!("Config file saved on {}", pathlight(location.as_ref()));
        Ok(())
    }

    /// Saves the changes made into the same path where the config was originally loaded.
    pub fn save_default(&self) -> Result<()> {
        self.save(Self::filepath(&self.path)?)
    }

    /// Performs the backup of the files in the different directories to the backup dir
    /// specified as root. It is advisable to use the same root for both the loading of the
    /// config file and the backup as it helps keep a per directory basis configuration.
    /// 
    /// In case of failure to backup one of the main directories specified in the config file
    /// this function will store the changes made up to that point and exit with an error. If
    /// the backup of one of the subelements fails the function will emit a warning and try
    /// to finish the rest of the process.
    /// 
    /// This function will only copy the needed files, if a file has not been modified since the
    /// last time it was backed up it will not be copied.
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

    /// Performs the restore of the backed files to their original locations on the specified
    /// root. As with the load function, it is advisable to use the same root from where the
    /// config file was loaded and uphold a per directory configuration.
    /// 
    /// The behaviour of this function is analogous to the backup function. If the restore of 
    /// one of the main directories fails. The function will exit with an error. If the restore
    /// of one of the subelements fails the function will emit a warning and continue restoring
    /// the other elements.
    /// 
    /// This function will only restore files that are newer or equal to the ones present in the
    /// original directory. This means that if you modify a file and has not been backed it will
    /// not be overriden by this function.
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

        debug!("config file: {}", pathlight(&restore));
        Ok(restore)
    }
}

/// Represents the structure of a folder in the config.json file.
/// 
/// This structure consists in a link between an origin (absolute path) and a
/// path relative to a specified root. The root will be typically the directory where
/// the config.json file is located but this is not obligatory.
/// 
/// Aside from the link, the modified field represents the last time the contents from
/// the two folders where synced
#[derive(Debug, Serialize, Deserialize)]
pub struct Folder {
    /// Link path. If thinked as a link, this is where the symbolic link is
    path: EnvPath,
    /// Path of origin. If thinked as a link, this is the place the link points to
    origin: EnvPath,
    /// Last time the folder was synced, if any. Parses from an RFC3339 valid string
    modified: Option<DateTime<Utc>>,
}

/// Represents the two dirs connected in a folder object once a root is given. Created
/// when a folder link gets 'resolved' by adding a root to the 'path' or 'link'
struct Dirs {
    rel: PathBuf,
    abs: PathBuf,
}

impl Folder {
    /// Resolves the link between the two elements in a folder. In order to do so a root
    /// must be given to the relative path
    fn resolve<P: AsRef<Path>>(&self, root: P) -> Dirs {
        Dirs {
            rel: root.as_ref().join(self.path.as_ref()),
            abs: PathBuf::from(self.origin.as_ref()),
        }
    }
}
