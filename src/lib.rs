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

use chrono::offset::Utc;
use chrono::{DateTime, SecondsFormat};
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

#[cfg(test)]
#[macro_use]
mod tools;

pub mod errors;
pub mod logger;
mod sync;

pub use errors::{AppError, AppErrorType};
use errors::{FsError, ParseError};
use logger::pathlight;
use sync::Direction;
use sync::FileSystemType;
use sync::Method;
use sync::ModelItem;
use sync::NewDirTree;
use sync::Presence;
use sync::TreeModel;
use sync::{OverwriteMode, SyncOptions};

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
    ///
    ///
    pub run: bool,
}

impl BackupOptions {
    /// Creates a new set of options for the backup operation.
    pub fn new(warn: bool, run: bool) -> Self {
        Self { warn, run }
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
    ///
    ///
    ///
    run: bool,
}

impl RestoreOptions {
    /// Creates a new set of options for the restore operation.
    pub fn new(warn: bool, overwrite: bool, run: bool) -> Self {
        Self {
            warn,
            overwrite,
            run,
        }
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

/// Represents a configuration file in json format. A valid json config file is composed
/// by an array of folder structs. The config file has to be stored in a subpath of the
/// directory being used.
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
        let file = dir.as_ref().join(path);
        debug!("Config file location: {}", pathlight(&file));
        let reader = File::open(&file).context(FsError::OpenFile((&file).into()))?;
        let folders = json::from_reader(reader).context(ParseError::JsonParse((&file).into()))?;
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
        let file = self.dir.as_ref().join(to);
        debug!("Config file location: {}", pathlight(&file));
        write!(
            File::create(&file).context(FsError::CreateFile((&file).into()))?,
            "{}",
            json::to_string_pretty(&self.folders).expect("ConfigFile cannot fail serialization")
        ).context(FsError::ReadFile((&file).into()))?;

        info!("Config file saved on {}", pathlight(file));
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
    ///
    /// Also, this function will delete files present in the backup that have been removed from
    /// their original locations and fail it cannot delete a file.
    pub fn backup(&mut self, options: BackupOptions) -> Result<DateTime<Utc>> {
        let stamp = Utc::now();

        let mut error = None;
        for folder in &mut self.folders {
            if let Err(err) = folder
                .backup(&self.dir, stamp, options)
                .context(AppErrorType::UpdateFolder)
            {
                error = Some(err);
                break;
            }
        }

        if options.run {
            if let Err(err) = self.save() {
                warn!(
                    "Unable to save on {} because of {}",
                    pathlight(self.dir.as_ref()),
                    err
                );
            }
        }

        match error {
            Some(err) => Err(err.into()),
            None => Ok(stamp),
        }
    }

    /// Performs the restore of the backed files on the dir where the config file was loaded to
    /// their original locations on the specified root.
    ///
    /// The behaviour of this function can be customized through the options provided. Check
    /// RestoreOptions to see what things can be modified.
    pub fn restore(self, options: RestoreOptions) -> Result<()> {
        for folder in &self.folders {
            folder
                .restore(&self.dir, options)
                .context(AppErrorType::RestoreFolder)?;
        }

        Ok(())
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
    /// Creates a new folder from the options specified
    #[cfg(test)]
    pub(self) fn new(path: EnvPath, origin: EnvPath, modified: Option<DateTime<Utc>>) -> Self {
        Self {
            path,
            origin,
            modified,
        }
    }

    /// Performs the backup of a specified folder entry. Given a root, the function checks
    /// for a previous backup and links all the files from the previous location, after
    /// that performs a sync operation between the folder and the origin location.
    pub(self) fn backup<P>(
        &mut self,
        root: P,
        stamp: DateTime<Utc>,
        options: BackupOptions,
    ) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let dirs = self.resolve(root);
        let (base, old, new) = if let Some(modified) = self.modified {
            (
                dirs.abs,
                Some(
                    dirs.rel
                        .join(modified.to_rfc3339_opts(SecondsFormat::Nanos, true)),
                ),
                dirs.rel
                    .join(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)),
            )
        } else {
            (
                dirs.abs,
                None,
                dirs.rel
                    .join(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)),
            )
        };

        let model: TreeModel = if let Some(ref old) = old {
            let tree = NewDirTree::new(&base, &old)?;
            tree.iter()
                .filter(|e| e.presence() != Presence::Dst)
                .map(|e| {
                    if e.kind() == FileSystemType::Dir {
                        ModelItem::new(base.join(e.path()), new.join(e.path()), Method::Dir)
                    } else if e.presence() == Presence::Src || !e.synced(Direction::Forward) {
                        ModelItem::new(base.join(e.path()), new.join(e.path()), Method::Copy)
                    } else {
                        ModelItem::new(old.join(e.path()), new.join(e.path()), Method::Link)
                    }
                }).collect()
        } else {
            let tree = NewDirTree::new(&base, &new)?;
            tree.iter()
                .map(|e| {
                    if e.kind() == FileSystemType::Dir {
                        ModelItem::new(base.join(e.path()), new.join(e.path()), Method::Dir)
                    } else {
                        ModelItem::new(base.join(e.path()), new.join(e.path()), Method::Copy)
                    }
                }).collect()
        };

        if options.run {
            model.execute()?;
            self.modified = Some(stamp);
        } else {
            model.log();
        }

        Ok(())
    }

    /// Performs the restore of the folder entry. Given a root, the function looks for the
    /// backup folder with the latest timestamp and performs the restore from there.
    pub(self) fn restore<P: AsRef<Path>>(&self, root: P, options: RestoreOptions) -> Result<()> {
        let mut dirs = self.resolve(root);
        if let Some(modified) = self.modified {
            debug!("Starting restore of: {}", pathlight(&dirs.rel));
            dirs.rel
                .push(modified.to_rfc3339_opts(SecondsFormat::Nanos, true));

            let tree = NewDirTree::new(&dirs.abs, &dirs.rel)?;
            let model: TreeModel = tree
                .iter()
                .filter(|e| {
                    e.presence() == Presence::Dst
                        || options.overwrite
                            && e.presence() == Presence::Both
                            && e.kind() != FileSystemType::Dir
                }).map(|e| {
                    if e.kind() == FileSystemType::Dir && e.presence() == Presence::Dst {
                        ModelItem::new(
                            dirs.rel.join(e.path()),
                            dirs.abs.join(e.path()),
                            Method::Dir,
                        )
                    } else {
                        ModelItem::new(
                            dirs.rel.join(e.path()),
                            dirs.abs.join(e.path()),
                            Method::Copy,
                        )
                    }
                }).collect();

            if options.run {
                model.execute().context(AppErrorType::RestoreFolder)?;
            } else {
                model.log();
            }

            Ok(())
        } else {
            info!("Restore not needed for {}", pathlight(&dirs.rel));
            Ok(())
        }
    }

    /// Resolves the link between the two elements in a folder. In order to do so a root
    /// must be given to the relative path.
    fn resolve<P: AsRef<Path>>(&self, root: P) -> Dirs {
        Dirs {
            rel: root.as_ref().join(self.path.as_ref()),
            abs: PathBuf::from(self.origin.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use {sync, BackupOptions, ConfigFile, Folder, RestoreOptions};

    macro_rules! rfc3339 {
        ($stamp:expr) => {{
            use chrono::SecondsFormat;
            $stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)
        }};
    }

    mod options {
        use super::{
            sync::{OverwriteMode, SyncOptions},
            BackupOptions, RestoreOptions,
        };

        #[test]
        fn test_backup_sync_options() {
            let backup = BackupOptions::new(true, true);
            let sync: SyncOptions = backup.clone().into();

            assert_eq!(sync.warn, backup.warn);
            assert!(sync.clean);
            assert_eq!(sync.overwrite, OverwriteMode::Allow);
        }

        #[test]
        fn test_restore_sync_options() {
            let restore = RestoreOptions::new(true, true, true);
            let sync: SyncOptions = restore.clone().into();

            assert_eq!(sync.warn, restore.warn);
            assert!(!sync.clean);
            assert_eq!(sync.overwrite, OverwriteMode::Force);

            let restore = RestoreOptions::new(true, false, true);
            let sync: SyncOptions = restore.clone().into();

            assert_eq!(sync.overwrite, OverwriteMode::Disallow);
        }
    }

    mod folder {
        use super::{BackupOptions, Folder, RestoreOptions};
        use chrono::offset::Utc;
        use env_path::EnvPath;
        use std::fs::{self, File, OpenOptions};
        use std::{env, io::Write, mem, path::PathBuf, thread, time};
        use tempfile;

        #[test]
        fn test_folder_resolve() {
            let home = env::var("HOME").expect("Unable to access $HOME var");
            let user = env::var("USER").expect("Unable to access $USER var");

            let folder = Folder {
                path: EnvPath::new("config"),
                origin: EnvPath::new(home.clone()),
                modified: None,
            };

            let dirs = folder.resolve(user.clone());

            assert_eq!(
                dirs.rel.display().to_string(),
                PathBuf::from(user.clone())
                    .join("config")
                    .display()
                    .to_string()
            );

            assert_eq!(
                dirs.abs.display().to_string(),
                PathBuf::from(home.clone()).display().to_string()
            );
        }

        #[test]
        fn test_folder_backup_single() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(false, true);

            Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            ).backup(root.path(), stamp, options)
            .expect("Unable to perform backup");

            let mut backup = tmppath!(&root, "backup");
            assert!(backup.exists());

            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(backup.join("a.txt").exists());
            assert!(backup.join("b.txt").exists());

            assert_eq!(read_file!(backup.join("a.txt")), "aaaa");
            assert_eq!(read_file!(backup.join("b.txt")), "bbbb");
        }

        #[test]
        fn test_folder_backup_double() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(false, true);
            let mut folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            );

            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            let mut backup = tmppath!(root, "backup");
            assert!(backup.exists());

            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(backup.join("a.txt").exists());
            assert!(backup.join("b.txt").exists());

            assert_eq!(read_file!(backup.join("a.txt")), "aaaa");
            assert_eq!(read_file!(backup.join("b.txt")), "bbbb");

            thread::sleep(time::Duration::from_millis(2000));
            let stamp = Utc::now();
            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            backup.pop();
            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(symlink!(backup.join("a.txt")));
            assert!(symlink!(backup.join("b.txt")));
        }

        #[test]
        fn test_folder_backup_double_addition() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(false, true);
            let mut folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            );

            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            let mut backup = root.path().join("backup");
            assert!(backup.exists());

            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(backup.join("a.txt").exists());
            assert!(backup.join("b.txt").exists());

            assert_eq!(read_file!(backup.join("a.txt")), "aaaa");
            assert_eq!(read_file!(backup.join("b.txt")), "bbbb");

            thread::sleep(time::Duration::from_millis(2000));

            // Create a new file in origin
            create_file!(tmppath!(origin, "c.txt"), "cccc");

            let stamp = Utc::now();
            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            backup.pop();
            backup.push(rfc3339!(stamp));

            assert!(backup.exists());

            assert!(symlink!(backup.join("a.txt")));
            assert!(symlink!(backup.join("b.txt")));
            assert!(filetype!(backup.join("c.txt")).is_file());

            assert_eq!(read_file!(backup.join("c.txt")), "cccc");
        }

        #[test]
        fn test_folder_backup_double_modification() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(false, true);
            let mut folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            );

            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            let mut backup = root.path().join("backup");
            assert!(backup.exists());

            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(backup.join("a.txt").exists());
            assert!(backup.join("b.txt").exists());

            assert_eq!(read_file!(backup.join("a.txt")), "aaaa");
            assert_eq!(read_file!(backup.join("b.txt")), "bbbb");

            thread::sleep(time::Duration::from_millis(2000));

            // Modify a file in origin
            let mut file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(origin.path().join("a.txt"))
                .expect("Unable to open file");
            write!(file, "cccc").unwrap();
            mem::drop(file);

            let stamp = Utc::now();
            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            backup.pop();
            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(filetype!(backup.join("a.txt")).is_file());
            assert!(symlink!(backup.join("b.txt")));

            assert_eq!(read_file!(backup.join("a.txt")), "aaaacccc");
        }

        #[test]
        fn test_folder_backup_double_remotion() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(false, true);
            let mut folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            );

            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            let mut backup = root.path().join("backup");
            assert!(backup.exists());

            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(backup.join("a.txt").exists());
            assert!(backup.join("b.txt").exists());

            assert_eq!(read_file!(backup.join("a.txt")), "aaaa");
            assert_eq!(read_file!(backup.join("b.txt")), "bbbb");

            thread::sleep(time::Duration::from_millis(2000));

            // Delete a file in origin
            fs::remove_file(tmppath!(origin, "a.txt")).unwrap();

            let stamp = Utc::now();
            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            backup.pop();
            backup.push(rfc3339!(stamp));

            assert!(backup.exists());
            assert!(!backup.join("a.txt").exists());
            assert!(symlink!(backup.join("b.txt")));
        }

        #[test]
        fn test_folder_restore_single() {
            let (origin, root) = (tmpdir!(), tmpdir!());
            let stamp = Utc::now();

            // Create some files on the backup
            let backup = tmppath!(root, format!("backup/{}", rfc3339!(stamp)));
            fs::create_dir_all(&backup).expect("Unable to create path");
            create_file!(backup.join("a.txt"), "aaaa");
            create_file!(backup.join("b.txt"), "bbbb");

            let folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                Some(stamp),
            );

            folder
                .restore(root.path(), RestoreOptions::new(false, true, true))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());

            assert_eq!(read_file!(tmppath!(origin, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(origin, "b.txt")), "bbbb");
        }

        #[test]
        fn test_folder_restore_with_symlinks() {
            let (origin, root) = (tmpdir!(), tmpdir!());
            let stamp = Utc::now();

            // Create some files on the backup
            let backup = tmppath!(root, format!("backup/{}", rfc3339!(stamp)));
            fs::create_dir_all(&backup).expect("Unable to create path");
            create_file!(backup.join("a.txt"), "aaaa");
            create_file!(backup.join("b.txt"), "bbbb");

            thread::sleep(time::Duration::from_millis(2000));
            let stamp_new = Utc::now();
            let backup_second = tmppath!(root, format!("backup/{}", rfc3339!(stamp_new)));
            fs::create_dir_all(&backup_second).expect("Unable to create path");

            // Create some symlinks
            #[cfg(unix)]
            use std::os::unix::fs::symlink;
            #[cfg(windows)]
            use std::os::windows::fs::symlink_file as symlink;

            symlink(backup.join("a.txt"), backup_second.join("a.txt")).unwrap();
            symlink(backup.join("b.txt"), backup_second.join("b.txt")).unwrap();

            let folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                Some(stamp_new),
            );

            folder
                .restore(root.path(), RestoreOptions::new(false, true, true))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());

            assert_eq!(read_file!(tmppath!(origin, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(origin, "b.txt")), "bbbb");
        }
    }

    mod config_file {
        use super::{BackupOptions, ConfigFile, RestoreOptions};
        use chrono::offset::Utc;
        use std::fs::{self, File};
        use std::io::Write;
        use {json, tempfile};

        #[test]
        fn test_config_file_load_valid() {
            let dir = tmpdir!();
            create_file!(
                tmppath!(dir, "config.json"),
                "[
                {{
                    \"path\": \"asd\", 
                    \"origin\": \"$HOME\", 
                    \"modified\": null
                }}
            ]"
            );
            assert!(ConfigFile::load_from(dir, "config.json").is_ok());
        }

        #[test]
        fn test_config_file_load_invalid() {
            let dir = tmpdir!();
            create_file!(
                tmppath!(dir, "config.json"),
                "[
                {{
                    \"path\": \"asd, 
                    \"origin\": \"$HOME\", 
                    \"modified\": null
                }}
            ]"
            );
            assert!(ConfigFile::load_from(dir, "config.json").is_err());
        }

        #[test]
        fn test_config_load() {
            let tmp = tmpdir!();
            fs::create_dir(tmppath!(tmp, ".backup")).expect("Unable to create folder");
            create_file!(
                tmppath!(tmp, ".backup/config.json"),
                "[
                {{
                    \"path\": \"backup\",
                    \"origin\": \"{}\",
                    \"modified\": null
                }}
            ]",
                tmppath!(tmp, "origin").display().to_string()
            );
            assert!(ConfigFile::load(tmp.path()).is_ok());
        }

        #[test]
        fn test_config_load_from() {
            let tmp = tempfile::tempdir().unwrap();

            let mut file = File::create(tmp.path().join("config.json")).unwrap();
            write!(
                file,
                "[
                {{
                    \"path\": \"backup\",
                    \"origin\": \"{}\",
                    \"modified\": null
                }}
            ]",
                tmp.path().join("origin").display().to_string()
            ).unwrap();

            let _config = ConfigFile::load_from(tmp.path(), "config.json").unwrap();
        }

        #[test]
        fn test_config_file_save_exists() {
            let dir = tmpdir!();
            assert!(create_file!(tmppath!(dir, "config.json")).exists());

            let config = ConfigFile {
                dir: dir.path(),
                folders: vec![],
            };

            assert!(config.save_to("config.json").is_ok());
        }

        #[test]
        fn test_config_file_save_unexistant() {
            let dir = tmpdir!();

            let config = ConfigFile {
                dir: dir.path(),
                folders: vec![],
            };

            assert!(config.save_to("config.json").is_ok());
        }

        #[test]
        fn test_config_save_to_format() {
            let tmp = tmpdir!();
            create_file!(
                tmppath!(tmp, "config.json"),
                "[
                {{
                    \"path\": \"backup\",
                    \"origin\": \"{}\",
                    \"modified\": null
                }}
            ]",
                tmppath!(tmp, "origin").display().to_string()
            );

            let config =
                ConfigFile::load_from(tmp.path(), "config.json").expect("Unable to load file");
            config
                .save_to("config2.json")
                .expect("Unable to save the file");

            assert_eq!(
                read_file!(tmppath!(tmp, "config2.json")),
                json::to_string_pretty(&config.folders).expect("Unable to serialize")
            );
        }

        #[test]
        fn test_config_save() {
            let tmp = tmpdir!();
            fs::create_dir(tmppath!(tmp, ".backup")).expect("Unable to create folder");

            let config = ConfigFile {
                dir: tmp.path(),
                folders: vec![],
            };
            config.save().expect("Unable to save");

            assert!(tmppath!(tmp, ".backup/config.json").exists());
        }

        #[test]
        fn test_config_backup() {
            let tmp = tmpdir!();
            let backup = tmppath!(tmp, "backup");

            fs::create_dir(tmppath!(tmp, "origin")).expect("Unable to create path");
            fs::create_dir_all(backup.join(".backup")).expect("Unable to create path");

            create_file!(
                backup.join(".backup/config.json"),
                "[
                {{
                    \"path\": \"backup\",
                    \"origin\": \"{origin}\",
                    \"modified\": null
                }},

                {{
                    \"path\": \"other\",
                    \"origin\": \"{origin}\",
                    \"modified\": null
                }}
            ]",
                origin = tmppath!(tmp, "origin").display().to_string()
            );

            let mut config = ConfigFile::load(&backup).expect("Unable to load file");
            let stamp = config
                .backup(BackupOptions::new(false, true))
                .expect("Unable to perform backup");

            assert!(backup.join(format!("backup/{}", rfc3339!(stamp))).exists());
            assert!(backup.join(format!("other/{}", rfc3339!(stamp))).exists());
        }

        #[test]
        fn test_config_restore() {
            let (origin, root) = (tmpdir!(), tmpdir!());
            let stamp = Utc::now();

            // Create the config file
            fs::create_dir(tmppath!(root, ".backup")).expect("Unable to create path");
            create_file!(
                tmppath!(root, ".backup/config.json"),
                "[
                {{
                    \"path\": \"backup\",
                    \"origin\": \"{}\",
                    \"modified\": \"{}\"
                }}
            ]",
                origin.path().display().to_string(),
                rfc3339!(stamp)
            );

            // Create some files on the backup
            let backup = tmppath!(root, format!("backup/{}", rfc3339!(stamp)));
            fs::create_dir_all(&backup).expect("Unable to create path");
            create_file!(backup.join("a.txt"));
            create_file!(backup.join("b.txt"));

            let config = ConfigFile::load(root.path()).expect("Unable to load file");
            config
                .restore(RestoreOptions::new(false, true, true))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());
        }
    }
}
