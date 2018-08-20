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
extern crate yansi;

extern crate env_path;

use chrono::offset::Utc;
use chrono::DateTime;
use env_path::EnvPath;
use failure::ResultExt;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Represents a failure in the execution of a function.
macro_rules! fail {
    ($x:expr) => {
        return Err($x);
    };
}

/// Used to build an error and then fail the current function execution with the builded
/// error.
macro_rules! err {
    ($x:expr) => {
        fail!(AppError::from($x))
    };
}

pub mod errors;
pub mod logger;
mod sync;

pub use errors::{AppError, AppErrorType};
use logger::pathlight;
use sync::{DirTree, OverwriteMode, SyncOptions};

/// Alias for the Result type
pub type Result<T> = ::std::result::Result<T, AppError>;

/// Modifier options for the backup action on ConfigFile. Check the properties to see which
/// behaviour they control
#[derive(Debug, Copy, Clone)]
pub struct BackupOptions {
    /// Enables/Disables warnings on the backup process. If an error is raises while processing
    /// the backup: a folder can't be read from (excluding the main folders), the user does not
    /// have permissions for accessing a file, the function will emit a warning instead of
    /// exiting with an error.
    ///
    /// In short words: (warn == true) => function will warn about errors instead of failing the
    /// backup operation
    pub warn: bool,
}

impl BackupOptions {
    /// Creates a new set of options for the backup operation.
    pub fn new(warn: bool) -> Self {
        Self { warn }
    }
}

impl From<BackupOptions> for SyncOptions {
    fn from(options: BackupOptions) -> Self {
        SyncOptions::new(options.warn, true, OverwriteMode::Allow)
    }
}

/// Modified options for the restore action on ConfigFile. Check the properties to see which
/// behaviour they control
#[derive(Debug, Copy, Clone)]
pub struct RestoreOptions {
    /// Enables/Disables warnings on the restore process. If an error is raised while processing
    /// the restore: a folder can't be read from (excluding the main folders), the user does not
    /// have permissions for accessing a file, the function will emit a warning instead of
    /// exiting with an error.
    ///
    /// In short words: (warn == true) => function will warn about errors instead of failing the
    /// backup operation.
    warn: bool,
    /// Enables/Disables overwrite on the original locations during the restore. If the original
    /// location of the file backed up already exists this function will overwrite the location
    /// with the file backed up instead of exiting with an error.
    ///
    /// In short words: (overwrite == true) => function wil overwrite files on the original
    /// locations.
    ///
    /// Setting (warn == true) will turn the error into a warning if (overwrite == false).
    overwrite: bool,
}

impl RestoreOptions {
    /// Creates a new set of options for the restore operation.
    pub fn new(warn: bool, overwrite: bool) -> Self {
        Self { warn, overwrite }
    }
}

impl From<RestoreOptions> for SyncOptions {
    fn from(options: RestoreOptions) -> Self {
        SyncOptions::new(
            options.warn,
            false,
            if options.overwrite {
                OverwriteMode::Force
            } else {
                OverwriteMode::Disallow
            },
        )
    }
}

/// Represents a configuration file stored in config.json. A valid config.json file is composed
/// by an array of folder structs. Usually this file will be stored in the directory intended
/// to be the backup directory. Even if it is technically possible to use single configuration
/// file to manage multiple backup dirs, this is not wise since the modified date for the folders
/// inside the config file will lose it's proper meaning.
///
/// This type is also the main point of entry for the library since it controls the
/// loading of the configuration and allows to do the ops related to the data inside, such
/// as the backup and restore of the files.
#[derive(Debug)]
pub struct ConfigFile<P>
where
    P: AsRef<Path> + Debug,
{
    dir: P,
    folders: Vec<Folder>,
}

impl<P> ConfigFile<P>
where
    P: AsRef<Path> + Debug,
{
    /// Represents the relative path to the configuration file from a given root directory
    const RESTORE: &'static str = ".backup/config.json";

    /// Loads the data present in the configuration file. Currently this function receives
    /// a directory and looks for the config file in the subpath .backup/config.json.
    pub fn load(dir: P) -> Result<Self> {
        Self::load_from(dir, Self::RESTORE)
    }

    pub fn load_from<T: AsRef<Path>>(dir: P, path: T) -> Result<Self> {
        let file = Self::filepath(&dir, &path, false)?;

        let reader =
            File::open(&file).context(AppErrorType::AccessFile(file.display().to_string()))?;
        let folders = json::from_reader(reader)
            .context(AppErrorType::JsonParse(file.display().to_string()))?;
        trace!("{:?}", folders);

        Ok(ConfigFile { dir, folders })
    }

    /// Saves the changes made back to the config.json file. Currently used more in a private
    /// fashion to update the last date when the folders were synced.
    ///
    /// This function uses the directory saved on the ConfigFile and looks for the
    /// config.json file inside .backup/config.json.
    pub fn save(&self) -> Result<()> {
        self.save_to(Self::RESTORE)
    }

    /// Saves the changes made to the path specified in 'to'. The 'to' path is relative to
    /// the master directory of ConfigFile. All the parent components of 'to' must exist
    /// in order for this function to suceed.
    pub fn save_to<T: AsRef<Path>>(&self, to: T) -> Result<()> {
        let path = Self::filepath(&self.dir, to, true)?;
        write!(
            File::create(&path).context(AppErrorType::AccessFile(path.display().to_string()))?,
            "{}",
            json::to_string_pretty(&self.folders).expect("ConfigFile cannot fail serialization")
        ).context(AppErrorType::AccessFile(path.display().to_string()))?;

        info!("Config file saved on {}", pathlight(path));
        Ok(())
    }

    /// Performs the backup of the files in the different directories to the backup dir
    /// where the config file was loaded.
    ///
    /// Behaviour of this function can be customized through the options provided. Check
    /// BackupOptions to see what things can be modified.
    ///
    /// This function will only copy the needed files, if a file has not been modified since the
    /// last time it was backed up it will not be copied.
    pub fn backup(&mut self, options: BackupOptions) -> Result<()> {
        let mut error = None;
        for folder in &mut self.folders {
            let dirs = folder.resolve(&self.dir);
            debug!("Starting backup of: {}", pathlight(&dirs.abs));

            if let Err(err) =
                DirTree::new(dirs.rel, dirs.abs)
                    .sync(options)
                    .context(AppErrorType::UpdateFolder(
                        self.dir.as_ref().display().to_string(),
                    )) {
                error = Some(err);
                break;
            }

            folder.modified = Some(Utc::now());
        }

        if let Err(err) = self.save() {
            warn!(
                "Unable to save on {} because of {}",
                pathlight(self.dir.as_ref()),
                err
            );
        }

        match error {
            Some(err) => Err(err.into()),
            None => Ok(()),
        }
    }

    /// Performs the restore of the backed files on the dir where the config file was loaded to
    /// their original locations on the specified root.
    ///
    /// The behaviour of this function is analogous to the backup function. If the restore of
    /// one of the main directories fails. The function will exit with an error. If the restore
    /// of one of the subelements fails the function will exit with an error if warn = false or
    /// emit a warning and continue restoring the other elements if warn = true.
    ///
    /// If overwrite = false, the function will exit with an error if the file exists in the
    /// original dir. If overwrite = true the function will only restore files that are newer or
    /// equal to the ones present in the original directory. This means that if you modify a file
    /// and has not been backed it will not be overriden by this function.
    pub fn restore(self, options: RestoreOptions) -> Result<()> {
        for folder in &self.folders {
            let dirs = folder.resolve(&self.dir);
            debug!("Starting restore of: {}", pathlight(&dirs.rel));

            DirTree::new(dirs.abs, dirs.rel)
                .sync(options)
                .context(AppErrorType::RestoreFolder(
                    self.dir.as_ref().display().to_string(),
                ))?;
        }

        Ok(())
    }

    /// Constructs the path to the configuration file from a given directory 'dir'. The
    /// path to add is the 'ext' path.
    /// 
    /// If allow is set to true, the function will allow the restore path to be unexistant.
    /// Note that the dir argument must still be a valid directory.
    fn filepath<T: AsRef<Path>, U: AsRef<Path>>(dir: T, ext: U, allow: bool) -> Result<PathBuf> {
        let path = dir.as_ref();

        if !path.is_dir() {
            err!(AppErrorType::NotDir(path.display().to_string()));
        }

        let restore = path.join(ext);
        if !restore.is_file() && !allow {
            err!(AppErrorType::PathUnexistant(restore.display().to_string()));
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
struct Folder {
    /// Link path. If thinked as a link, this is where the symbolic link is
    path: EnvPath,
    /// Path of origin. If thinked as a link, this is the place the link points to
    origin: EnvPath,
    /// Last time the folder was synced, if any. Parses from an RFC3339 valid string
    modified: Option<DateTime<Utc>>,
}

/// Represents the two dirs connected in a folder object once a root is given. Created
/// when a folder link gets 'resolved' by adding a root to the 'path' or 'link'
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    extern crate tempfile;

    use env_path::EnvPath;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use sync::{OverwriteMode, SyncOptions};
    use {BackupOptions, ConfigFile, Folder, RestoreOptions};

    #[test]
    fn test_backup_sync_options() {
        let backup = BackupOptions::new(true);
        let sync: SyncOptions = backup.clone().into();

        assert_eq!(sync.warn, backup.warn);
        assert_eq!(sync.clean, true);
        assert_eq!(sync.overwrite, OverwriteMode::Allow);
    }

    #[test]
    fn test_restore_sync_options() {
        let restore = RestoreOptions::new(true, true);
        let sync: SyncOptions = restore.clone().into();

        assert_eq!(sync.warn, restore.warn);
        assert_eq!(sync.clean, false);
        assert_eq!(sync.overwrite, OverwriteMode::Force);

        let restore = RestoreOptions::new(true, false);
        let sync: SyncOptions = restore.clone().into();

        assert_eq!(sync.overwrite, OverwriteMode::Disallow);
    }

    #[test]
    fn test_config_file_load_valid() {
        let dir = tempfile::tempdir().expect("Creation of temp dir failed");
        let mut tmpfile = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(dir.path().join("config.json"))
            .expect("Unable to create tmp file");

        write!(
            tmpfile,
            "[{{\"path\": \"asd\", \"origin\": \"$HOME\", \"modified\": null}}]"
        ).expect("Unable to write on tmp file");

        assert!(
            ConfigFile::load_from(dir, "config.json").is_ok(),
            "Unable to load configuration"
        );
    }

    #[test]
    fn test_config_file_load_invalid() {
        let dir = tempfile::tempdir().expect("Creation of temp dir failed");
        let mut tmpfile = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(dir.path().join("config.json"))
            .expect("Unable to create tmp file");

        write!(
            tmpfile,
            "[{{\"path\": \"asd, \"origin\": \"$HOME\", \"modified\": null}}]"
        ).expect("Unable to write on tmp file");

        assert!(
            ConfigFile::load_from(dir, "config.json").is_err(),
            "Unable to load configuration"
        );
    }

    #[test]
    fn test_config_file_save_exists() {
        let dir = tempfile::tempdir().expect("Creation of temp dir failed");
        let _tmpfile = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(dir.path().join("config.json"))
            .expect("Unable to create tmp file");

        assert!(
            dir.path().join("config.json").exists(),
            "Save file was not created"
        );

        let config = ConfigFile {
            dir: dir.path(),
            folders: vec![],
        };

        assert!(
            config.save_to("config.json").is_ok(),
            "Unable to save into location"
        );
    }

    #[test]
    fn test_config_file_save_unexistant() {
        let dir = tempfile::tempdir().expect("Creation of temp dir failed");

        let config = ConfigFile {
            dir: dir.path(),
            folders: vec![],
        };

        assert!(
            config.save_to("config.json").is_ok(),
            "Unable to save into location"
        );
    }

    #[test]
    fn test_folder_resolve() {
        let folder = Folder {
            path: EnvPath::new("config"),
            origin: EnvPath::new(env::var("HOME").unwrap()),
            modified: None,
        };

        let dirs = folder.resolve(env::var("USER").unwrap());

        assert_eq!(
            dirs.rel.display().to_string(),
            PathBuf::from(env::var("USER").unwrap())
                .join("config")
                .display()
                .to_string()
        );

        assert_eq!(
            dirs.abs.display().to_string(),
            PathBuf::from(env::var("HOME").unwrap())
                .display()
                .to_string()
        );
    }
}
