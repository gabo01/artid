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

#[cfg(test)]
#[macro_use]
mod tools;

pub mod errors;
pub mod logger;
mod sync;

pub use errors::{AppError, AppErrorType};
use errors::{FsError, ParseError};
use logger::pathlight;
use sync::{CopyAction, CopyModel, DirTree, Direction, FileType, Presence};

/// Alias for the Result type
pub type Result<T> = ::std::result::Result<T, AppError>;

/// Modifier options for the backup action on ConfigFile. Check the properties to see which
/// behaviour they control
#[derive(Debug, Copy, Clone)]
pub struct BackupOptions {
    /// Controls if the model should be ran or not. In case the model does not run, the
    /// intended actions will be logged into the screen
    pub run: bool,
}

impl BackupOptions {
    /// Creates a new set of options for the backup operation.
    pub fn new(run: bool) -> Self {
        Self { run }
    }
}

/// Modified options for the restore action on ConfigFile. Check the properties to see which
/// behaviour they control
#[derive(Debug, Copy, Clone)]
pub struct RestoreOptions {
    /// Enables/Disables overwrite on the original locations during the restore. If the original
    /// location of the file backed up already exists this function will overwrite the location
    /// with the file backed up instead of exiting with an error.
    ///
    /// In short words: (overwrite == true) => function wil overwrite files on the original
    /// locations.
    overwrite: bool,
    /// Controls if the model should be ran or not. In case the model does not run, the
    /// intended actions will be logged into the screen
    run: bool,
}

impl RestoreOptions {
    /// Creates a new set of options for the restore operation.
    pub fn new(overwrite: bool, run: bool) -> Self {
        Self { overwrite, run }
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

    /// Manually create a new ConfigFile object. Usually, you would load (see load method) the
    /// configuration file from disk, but in certain cases like directory initialization it can
    /// be useful to create the file manually
    pub fn new(dir: P) -> Self {
        Self {
            dir,
            folders: vec![],
        }
    }

    /// Returns a reference to the folders in the configuration file
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }

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
#[derive(Debug)]
pub struct Dirs {
    rel: PathBuf,
    abs: PathBuf,
}

impl Folder {
    /// Creates a new folder from the options specified
    pub fn new(path: EnvPath, origin: EnvPath, modified: Option<DateTime<Utc>>) -> Self {
        Self {
            path,
            origin,
            modified,
        }
    }

    /// Performs the backup of a specified folder entry. Given a root, the function checks
    /// for a previous backup and links all the files from the previous location, after
    /// that performs a sync operation between the folder and the origin location.
    fn backup<P>(&mut self, root: P, stamp: DateTime<Utc>, options: BackupOptions) -> Result<()>
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

        let model: CopyModel = if let Some(ref old) = old {
            let tree = DirTree::new(&base, &old)?;
            tree.iter()
                .filter(|e| e.presence() != Presence::Dst)
                .map(|e| {
                    if e.kind() == FileType::Dir {
                        CopyAction::CreateDir {
                            target: new.join(e.path()),
                        }
                    } else if e.presence() == Presence::Src || !e.synced(Direction::Forward) {
                        CopyAction::CopyFile {
                            src: base.join(e.path()),
                            dst: new.join(e.path()),
                        }
                    } else {
                        CopyAction::CopyLink {
                            src: old.join(e.path()),
                            dst: new.join(e.path()),
                        }
                    }
                }).collect()
        } else {
            let tree = DirTree::new(&base, &new)?;
            tree.iter()
                .map(|e| {
                    if e.kind() == FileType::Dir {
                        CopyAction::CreateDir {
                            target: new.join(e.path()),
                        }
                    } else {
                        CopyAction::CopyFile {
                            src: base.join(e.path()),
                            dst: new.join(e.path()),
                        }
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
    fn restore<P: AsRef<Path>>(&self, root: P, options: RestoreOptions) -> Result<()> {
        let mut dirs = self.resolve(root);
        if let Some(modified) = self.modified {
            debug!("Starting restore of: {}", pathlight(&dirs.rel));
            dirs.rel
                .push(modified.to_rfc3339_opts(SecondsFormat::Nanos, true));

            let tree = DirTree::new(&dirs.abs, &dirs.rel)?;
            let model: CopyModel = tree
                .iter()
                .filter(|e| {
                    e.presence() == Presence::Dst
                        || options.overwrite
                            && e.presence() == Presence::Both
                            && e.kind() != FileType::Dir
                }).map(|e| {
                    if e.kind() == FileType::Dir && e.presence() == Presence::Dst {
                        CopyAction::CreateDir {
                            target: dirs.abs.join(e.path()),
                        }
                    } else {
                        CopyAction::CopyFile {
                            src: dirs.rel.join(e.path()),
                            dst: dirs.abs.join(e.path()),
                        }
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
    use {BackupOptions, Folder, RestoreOptions};

    macro_rules! rfc3339 {
        ($stamp:expr) => {{
            use chrono::SecondsFormat;
            $stamp.to_rfc3339_opts(SecondsFormat::Nanos, true)
        }};
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
            let options = BackupOptions::new(true);

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
        #[ignore]
        fn test_folder_backup_double() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(true);
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
        #[ignore]
        fn test_folder_backup_double_addition() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(true);
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
            let options = BackupOptions::new(true);
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
        #[ignore]
        fn test_folder_backup_double_remotion() {
            let origin = tmpdir!();
            create_file!(tmppath!(origin, "a.txt"), "aaaa");
            create_file!(tmppath!(origin, "b.txt"), "bbbb");

            let root = tmpdir!();

            let stamp = Utc::now();
            let options = BackupOptions::new(true);
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
                .restore(root.path(), RestoreOptions::new(true, true))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());

            assert_eq!(read_file!(tmppath!(origin, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(origin, "b.txt")), "bbbb");
        }

        #[test]
        #[ignore]
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
                .restore(root.path(), RestoreOptions::new(true, true))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());

            assert_eq!(read_file!(tmppath!(origin, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(origin, "b.txt")), "bbbb");
        }
    }
}
