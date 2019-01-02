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

mod errors;
mod ops;

pub use self::{
    errors::FileError,
    ops::{BackupOptions, RestoreOptions},
    ops::{OperativeError, OperativeErrorType},
};

use self::{
    errors::FileErrorType,
    ops::{Backup, Restore},
};

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
    folders: Vec<FolderConfig>,
}

impl<P> ConfigFile<P>
where
    P: AsRef<Path> + Debug,
{
    /// Represents the relative path to the configuration file from a given root directory
    const SAVE_PATH: &'static str = ".backup/config.json";

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
    pub fn folders(&self) -> &[FolderConfig] {
        &self.folders
    }

    /// Loads the data present in the configuration file. Currently this function receives
    /// a directory and looks for the config file in the subpath .backup/config.json.
    pub fn load(dir: P) -> Result<Self, FileError> {
        Self::load_from(dir, Self::SAVE_PATH)
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
        self.save_to(Self::SAVE_PATH)
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
        )
        .context(FileErrorType::Save(file.display().to_string()))?;

        info!("Config file saved on {}", pathlight(file));
        Ok(())
    }

    /// Links and returns the list of folders present in the configuration
    pub fn list_folders(&mut self) -> Vec<FileSystemFolder<'_>> {
        let root = &self.dir;
        self.folders
            .iter_mut()
            .map(|folder| folder.apply_root(&root))
            .collect()
    }

    /// Returns the folder with a path matching the given path. Comparison will be done based
    /// on the relative path.
    pub fn get_folder<T: AsRef<Path>>(&mut self, path: T) -> Option<FileSystemFolder<'_>> {
        let root = &self.dir;

        self.folders
            .iter_mut()
            .find(|folder| folder.path.path() == path.as_ref())
            .map(|x| x.apply_root(&root))
    }

    /// Performs the backup of the files in the different directories to the backup dir
    /// where the config file was loaded.
    ///
    /// Behaviour of this function can be customized through the options provided. Check
    /// BackupOptions to see what things can be modified.
    ///
    /// This function will only copy the needed files, if a file has not been modified since the
    /// last time it was backed up it will not be copied.
    pub fn backup(&mut self, options: BackupOptions) -> Result<DateTime<Utc>, OperativeError> {
        let stamp = Utc::now();

        let mut error = None;
        for folder in &mut self.folders {
            if let Err(err) = folder.apply_root(&self.dir).backup(stamp, options) {
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
pub struct FolderConfig {
    /// Link path. If thinked as a link, this is where the symbolic link is
    path: EnvPath,
    /// Path of origin. If thinked as a link, this is the place the link points to
    origin: EnvPath,
    /// Last time the folder was synced, if any. Parses from an RFC3339 valid string
    modified: Option<Vec<DateTime<Utc>>>,
}

impl FolderConfig {
    /// Creates a new folder from the options specified
    pub fn new<P: Into<PathBuf>, T: Into<PathBuf>>(path: P, origin: T) -> Self {
        Self {
            path: EnvPath::new(path.into().display().to_string()),
            origin: EnvPath::new(origin.into().display().to_string()),
            modified: None,
        }
    }

    /// Checks if there has been a sync of the folder
    pub fn has_sync(&self) -> bool {
        match self.modified {
            Some(ref vec) if !vec.is_empty() => true,
            Some(_) | None => false,
        }
    }

    /// Returns the last sync of the folder if there is one
    pub fn find_last_sync(&self) -> Option<DateTime<Utc>> {
        match self.modified {
            Some(ref vec) if !vec.is_empty() => Some(vec.last().unwrap().to_owned()),
            Some(_) | None => None,
        }
    }

    /// Returns the backup date, if it exists, of the position specified by point
    pub fn find_sync(&self, point: usize) -> Option<DateTime<Utc>> {
        match self.modified {
            Some(ref vec) => vec.get(point).map(ToOwned::to_owned),
            None => None,
        }
    }

    /// Register a new folder backup on the list
    fn add_modified(&mut self, stamp: DateTime<Utc>) {
        match self.modified {
            Some(ref mut vec) => vec.push(stamp),
            None => self.modified = Some(vec![stamp]),
        }
    }

    /// Searches for the two folders and links them with the details present here into a new
    /// type
    fn apply_root<P: AsRef<Path>>(&mut self, root: P) -> FileSystemFolder<'_> {
        let origin = self.origin.as_ref().into();
        let relative = root.as_ref().join(self.path.as_ref());

        FileSystemFolder::new(self, origin, relative)
    }
}

/// Represents two linked directories on the filesystem.
#[derive(Debug)]
struct Link {
    origin: PathBuf,
    relative: PathBuf,
}

/// Represents a two point directory on the filesystem with the details present in the
/// configuration file added.
#[derive(Debug)]
pub struct FileSystemFolder<'a> {
    link: Link,
    config: &'a mut FolderConfig,
}

impl<'a> FileSystemFolder<'a> {
    /// Builds a new link from two directories to link and a configuration folder
    fn new(config: &'a mut FolderConfig, origin: PathBuf, relative: PathBuf) -> Self {
        Self {
            config,
            link: Link { origin, relative },
        }
    }

    /// Performs the backup of a specified folder entry. The function checks
    /// for a previous backup and links all the files from the previous location, after
    /// that performs a sync operation between the folder and the origin location.
    pub fn backup(
        &mut self,
        stamp: DateTime<Utc>,
        options: BackupOptions,
    ) -> Result<(), OperativeError> {
        let model = if let Some(modified) = self.config.find_last_sync() {
            let old = self.link.relative.join(rfc3339!(modified));
            let new = self.link.relative.join(rfc3339!(stamp));
            Backup::with_previous(&self.link.origin, &old, &new)?
        } else {
            let relative = self.link.relative.join(rfc3339!(stamp));
            Backup::from_scratch(&self.link.origin, &relative)?
        };

        if options.run {
            model.execute().context(OperativeErrorType::Backup)?;
            self.config.add_modified(stamp);
        } else {
            model.log();
        }

        Ok(())
    }

    /// Performs the restore of the folder entry.The function looks for the
    /// backup folder with the latest timestamp and performs the restore from there.
    pub fn restore(&self, options: RestoreOptions) -> Result<(), OperativeError> {
        if self.config.has_sync() {
            let modified = match options.point {
                Some(point) => self.config.find_sync(point),
                None => self.config.find_last_sync(),
            };

            if let Some(modified) = modified {
                debug!("Starting restore of: {}", pathlight(&self.link.relative));
                let relative = self.link.relative.join(rfc3339!(modified));

                let model = Restore::from_point(&self.link.origin, &relative, options.overwrite)?;

                if options.run {
                    model.execute().context(OperativeErrorType::Restore)?;
                } else {
                    model.log();
                }

                Ok(())
            } else {
                Err(OperativeErrorType::PointDoesNotExists)?
            }
        } else {
            info!("Restore not needed for {}", pathlight(&self.link.relative));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use prelude::{BackupOptions, FolderConfig, RestoreOptions};

    mod folder {
        use super::{BackupOptions, FolderConfig, RestoreOptions};
        use chrono::offset::Utc;
        use std::fs::{self, File, OpenOptions};
        use std::{env, io::Write, mem, path::PathBuf, thread, time};
        use tempfile;

        #[test]
        fn test_folder_resolve() {
            let home = env::var("HOME").expect("Unable to access $HOME var");
            let user = env::var("USER").expect("Unable to access $USER var");

            let mut config = FolderConfig::new("config", home.clone());
            let folder = config.apply_root(user.clone());

            assert_eq!(
                folder.link.relative.display().to_string(),
                PathBuf::from(user.clone())
                    .join("config")
                    .display()
                    .to_string()
            );

            assert_eq!(
                folder.link.origin.display().to_string(),
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

            let mut config = FolderConfig::new("backup", origin.path());
            let mut folder = config.apply_root(root.path());

            folder
                .backup(stamp, options)
                .expect("Unable to perform backup");

            let mut backup = tmppath!(&root, "backup");
            assert!(backup.exists());

            assert_eq!(folder.config.modified, Some(vec![stamp]));

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

            let mut config = FolderConfig::new("backup", origin.path());
            let mut folder = config.apply_root(root.path());

            folder
                .backup(stamp, options)
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
                .backup(stamp, options)
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

            let mut config = FolderConfig::new("backup", origin.path());
            let mut folder = config.apply_root(root.path());

            folder
                .backup(stamp, options)
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
                .backup(stamp, options)
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

            let mut config = FolderConfig::new("backup", origin.path());
            let mut folder = config.apply_root(root.path());

            folder
                .backup(stamp, options)
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
                .backup(stamp, options)
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

            let mut config = FolderConfig::new("backup", origin.path());
            let mut folder = config.apply_root(root.path());

            folder
                .backup(stamp, options)
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
                .backup(stamp, options)
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

            let mut config = FolderConfig::new("backup", origin.path());
            config.modified = Some(vec![stamp]);
            let folder = config.apply_root(root.path());

            folder
                .restore(RestoreOptions::new(true, true, None))
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

            let mut config = FolderConfig::new("backup", origin.path());
            config.modified = Some(vec![stamp_new]);
            let folder = config.apply_root(root.path());

            folder
                .restore(RestoreOptions::new(true, true, None))
                .expect("Unable to perform restore");

            assert!(tmppath!(origin, "a.txt").exists());
            assert!(tmppath!(origin, "b.txt").exists());

            assert_eq!(read_file!(tmppath!(origin, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(origin, "b.txt")), "bbbb");
        }
    }
}
