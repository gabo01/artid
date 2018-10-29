//! The config module is responsible for managing everything related to the configuration
//! file.
//!
//! The configuration file is currently a config.json file where an array of folders is stored.
//! Each folder represents both a folder on the backup directory and an origin folder on
//! someplace, usually outside the backup directory.
//!
//! Most of artid's operations such as backup, restore and zip are applied to a single folder
//! so they depend on the folder's given configuration on the file to do their work.

use chrono::offset::Utc;
use chrono::DateTime;
use env_path::EnvPath;
use failure::ResultExt;
use json;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use logger::pathlight;
use ops::{Backup, BackupOptions, OperativeError, OperativeErrorType, Restore, RestoreOptions};

mod errors;
pub use self::errors::FileError;
use self::errors::FileErrorType;

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
    pub fn load(dir: P) -> Result<Self, FileError> {
        Self::load_from(dir, Self::RESTORE)
    }

    /// Loads the data present in the configuration file present in path. A thing to notice
    /// is that path must be relative to the root used to create the object.
    pub fn load_from<T: AsRef<Path>>(dir: P, path: T) -> Result<Self, FileError> {
        let file = dir.as_ref().join(path);

        debug!("Config file location: {}", pathlight(&file));

        let reader = File::open(&file).context(FileErrorType::Load(file.display().to_string()))?;
        let folders =
            json::from_reader(reader).context(FileErrorType::Parse(file.display().to_string()))?;
        trace!("{:#?}", folders);

        Ok(ConfigFile { dir, folders })
    }

    /// Saves the changes made back to the config.json file. Currently used more in a private
    /// fashion to update the last date when the folders were synced.
    ///
    /// This function uses the directory saved on the ConfigFile and looks for the
    /// config.json file inside .backup/config.json.
    pub fn save(&self) -> Result<(), FileError> {
        self.save_to(Self::RESTORE)
    }

    /// Saves the changes made to the path specified in 'to'. The 'to' path is relative to
    /// the master directory of ConfigFile. All the parent components of 'to' must exist
    /// in order for this function to suceed.
    pub fn save_to<T: AsRef<Path>>(&self, to: T) -> Result<(), FileError> {
        let file = self.dir.as_ref().join(to);

        debug!("Config file location: {}", pathlight(&file));

        write!(
            File::create(&file).context(FileErrorType::Save(file.display().to_string()))?,
            "{}",
            json::to_string_pretty(&self.folders).expect("ConfigFile cannot fail serialization")
        ).context(FileErrorType::Save(file.display().to_string()))?;

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
    pub fn backup(&mut self, options: BackupOptions) -> Result<DateTime<Utc>, OperativeError> {
        let stamp = Utc::now();

        let mut error = None;
        for folder in &mut self.folders {
            if let Err(err) = folder.backup(&self.dir, stamp, options) {
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
            Some(err) => Err(err),
            None => Ok(stamp),
        }
    }

    /// Performs the restore of the backed files on the dir where the config file was loaded to
    /// their original locations on the specified root.
    ///
    /// The behaviour of this function can be customized through the options provided. Check
    /// RestoreOptions to see what things can be modified.
    pub fn restore(self, options: RestoreOptions) -> Result<(), OperativeError> {
        for folder in &self.folders {
            folder.restore(&self.dir, options)?;
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
    fn backup<P>(
        &mut self,
        root: P,
        stamp: DateTime<Utc>,
        options: BackupOptions,
    ) -> Result<(), OperativeError>
    where
        P: AsRef<Path>,
    {
        let (rel, abs) = self.resolve(root);

        let model = if let Some(modified) = self.modified {
            let (old, new) = (rel.join(rfc3339!(modified)), rel.join(rfc3339!(stamp)));
            Backup::with_previous(&abs, &old, &new)?
        } else {
            Backup::from_scratch(&abs, &rel.join(rfc3339!(stamp)))?
        };

        if options.run {
            model.execute().context(OperativeErrorType::Backup)?;
            self.modified = Some(stamp);
        } else {
            model.log();
        }

        Ok(())
    }

    /// Performs the restore of the folder entry. Given a root, the function looks for the
    /// backup folder with the latest timestamp and performs the restore from there.
    fn restore<P: AsRef<Path>>(
        &self,
        root: P,
        options: RestoreOptions,
    ) -> Result<(), OperativeError> {
        let (mut rel, abs) = self.resolve(root);
        if let Some(modified) = self.modified {
            debug!("Starting restore of: {}", pathlight(&rel));
            rel.push(rfc3339!(modified));

            let model = Restore::from_point(&abs, &rel, options.overwrite)?;

            if options.run {
                model.execute().context(OperativeErrorType::Restore)?;
            } else {
                model.log();
            }

            Ok(())
        } else {
            info!("Restore not needed for {}", pathlight(&rel));
            Ok(())
        }
    }

    /// Resolves the link between the two elements in a folder. In order to do so a root
    /// must be given to the relative path.
    ///
    /// The returned elements are the backup path and the origin path respectively. Can also
    /// be seen as the resolved relative path and the absolute path
    fn resolve<P: AsRef<Path>>(&self, root: P) -> (PathBuf, PathBuf) {
        (
            root.as_ref().join(self.path.as_ref()),
            PathBuf::from(self.origin.as_ref()),
        )
    }
}

#[cfg(test)]
mod tests {
    use prelude::{BackupOptions, Folder, RestoreOptions};

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

            let (rel, abs) = folder.resolve(user.clone());

            assert_eq!(
                rel.display().to_string(),
                PathBuf::from(user.clone())
                    .join("config")
                    .display()
                    .to_string()
            );

            assert_eq!(
                abs.display().to_string(),
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

            let mut folder = Folder::new(
                EnvPath::new("backup"),
                EnvPath::new(origin.path().display().to_string()),
                None,
            );

            folder
                .backup(root.path(), stamp, options)
                .expect("Unable to perform backup");

            let mut backup = tmppath!(&root, "backup");
            assert!(backup.exists());

            assert_eq!(folder.modified, Some(stamp));

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
