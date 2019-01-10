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
    pub(crate) dir: P,
    pub(crate) folders: Vec<FolderConfig>,
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
        debug!("Config file location: '{}'", file.display());

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

        debug!("Config file location: '{}'", file.display());

        write!(
            File::create(&file).context(FileErrorType::Save(file.display().to_string()))?,
            "{}",
            json::to_string_pretty(&self.folders).expect("ConfigFile cannot fail serialization")
        )
        .context(FileErrorType::Save(file.display().to_string()))?;

        info!("Config file saved on '{}'", file.display());
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
    pub(crate) path: EnvPath,
    /// Path of origin. If thinked as a link, this is the place the link points to
    pub(crate) origin: EnvPath,
    /// Last time the folder was synced, if any. Parses from an RFC3339 valid string
    pub(crate) modified: Option<Vec<DateTime<Utc>>>,
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
    pub(crate) fn add_modified(&mut self, stamp: DateTime<Utc>) {
        match self.modified {
            Some(ref mut vec) => vec.push(stamp),
            None => self.modified = Some(vec![stamp]),
        }
    }

    /// Searches for the two folders and links them with the details present here into a new
    /// type
    pub(crate) fn apply_root<P: AsRef<Path>>(&mut self, root: P) -> FileSystemFolder<'_> {
        let origin = self.origin.as_ref().into();
        let relative = root.as_ref().join(self.path.as_ref());

        FileSystemFolder::new(self, origin, relative)
    }
}

/// Represents two linked directories on the filesystem.
#[derive(Debug)]
pub(crate) struct Link {
    pub(crate) origin: PathBuf,
    pub(crate) relative: PathBuf,
}

/// Represents a two point directory on the filesystem with the details present in the
/// configuration file added.
#[derive(Debug)]
pub struct FileSystemFolder<'a> {
    pub(crate) link: Link,
    pub(crate) config: &'a mut FolderConfig,
}

impl<'a> FileSystemFolder<'a> {
    /// Builds a new link from two directories to link and a configuration folder
    fn new(config: &'a mut FolderConfig, origin: PathBuf, relative: PathBuf) -> Self {
        Self {
            config,
            link: Link { origin, relative },
        }
    }
}

#[cfg(test)]
mod tests {
    use prelude::{ConfigFile, FolderConfig};

    mod config {
        use super::ConfigFile;
        use chrono::Utc;
        use json;
        use std::fs::{self, File};
        use std::io::Write;
        use tempfile;

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
        fn test_config_file_load_valid_with_modified() {
            let dir = tmpdir!();
            create_file!(
                tmppath!(dir, "config.json"),
                "[
                {{
                    \"path\": \"asd\", 
                    \"origin\": \"$HOME\", 
                    \"modified\": [\"{}\"]
                }}
            ]",
                rfc3339!(Utc::now())
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
            )
            .unwrap();

            let _config = ConfigFile::load_from(tmp.path(), "config.json").unwrap();
        }

        #[test]
        fn test_config_file_save_exists() {
            let dir = tmpdir!();
            assert!(create_file!(tmppath!(dir, "config.json")).exists());

            let config = ConfigFile::new(dir.path());
            assert!(config.save_to("config.json").is_ok());
        }

        #[test]
        fn test_config_file_save_unexistant() {
            let dir = tmpdir!();

            let config = ConfigFile::new(dir.path());
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
                json::to_string_pretty(config.folders()).expect("Cannot fail serialization"),
                read_file!(tmppath!(tmp, "config2.json")),
            );
        }

        #[test]
        fn test_config_save() {
            let tmp = tmpdir!();
            fs::create_dir(tmppath!(tmp, ".backup")).expect("Unable to create folder");

            let config = ConfigFile::new(tmp.path());
            config.save().expect("Unable to save");

            assert!(tmppath!(tmp, ".backup/config.json").exists());
        }
    }

    mod folder {
        use super::FolderConfig;
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
    }
}
