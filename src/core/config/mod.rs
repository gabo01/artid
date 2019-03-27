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
use log::{debug, info, log, trace};
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub mod archive;
mod errors;

use self::archive::{Archive, History};
pub use self::errors::FileError;
use self::errors::FileErrorType;

/// Represents the whole archive located in a folder. As such, interaction with the archive
/// is mostly done through this type
#[derive(Debug)]
pub struct ArtidArchive<P>
where
    P: AsRef<Path> + Debug,
{
    pub(crate) folder: P,
    pub(crate) archive: Archive,
}

impl<P: AsRef<Path> + Debug> ArtidArchive<P> {
    /// Represents the relative path to the configuration file from a given root directory
    const SAVE_PATH: &'static str = ".artid/artid.toml";

    /// Creates a new empty archive in the folder P. The created archive, useful for
    /// intialization purpouses, is stored only in memory and must be saved to disk separately
    pub fn new(folder: P) -> Self {
        Self {
            folder,
            archive: Archive::default(),
        }
    }

    /// Makes the archive aware of a new folder in disk. The new folder will be represented
    /// by the path it takes inside the archive and it's origin path in disk. At this point,
    /// an id will be assigned to the folder to uniquely identify it even if it's path of
    /// origin or it's path inside the archive changes
    pub fn add_folder<PS, O>(&mut self, path: PS, origin: O)
    where
        PS: Into<String>,
        O: Into<String>,
    {
        self.archive.add_folder(path, origin)
    }

    /// Find the id of a folder based on it's relative path
    pub fn get_folder_id(&self, path: &str) -> Option<String> {
        self.archive.get_folder_id(path)
    }

    /// Returns the set of snapshots stored in the archive
    pub fn history(&self) -> &History {
        &self.archive.history
    }

    /// Loads the archive present inside the folder P. This function looks for the archive
    /// configuration inside the default SAVE_PATH
    pub fn load(folder: P) -> Result<Self, FileError> {
        Self::load_from(folder, Self::SAVE_PATH)
    }

    /// Loads the archive present inside the folder P with a custom path for the archive
    /// configuration
    pub fn load_from<T: AsRef<Path>>(folder: P, path: T) -> Result<Self, FileError> {
        let file = folder.as_ref().join(path);
        debug!("Config file location: '{}'", file.display());

        let contents =
            fs::read_to_string(&file).context(FileErrorType::Load(file.display().to_string()))?;
        let archive =
            toml::from_str(&contents).context(FileErrorType::Parse(file.display().to_string()))?;
        trace!("{:#?}", archive);

        Ok(Self { folder, archive })
    }

    /// Saves the archive to the disk. It uses the default SAVE_PATH to save the global archive
    /// configuration
    pub fn save(&self) -> Result<(), FileError> {
        self.save_to(Self::SAVE_PATH)
    }

    /// Saves the archive to the disk using a custom path T to save the global configuration
    pub fn save_to<T: AsRef<Path>>(&self, to: T) -> Result<(), FileError> {
        let file = self.folder.as_ref().join(to);

        debug!("Config file location: '{}'", file.display());

        write!(
            File::create(&file).context(FileErrorType::Save(file.display().to_string()))?,
            "{}",
            toml::to_string_pretty(&self.archive).expect("Archive cannot fail serialization")
        )
        .context(FileErrorType::Save(file.display().to_string()))?;

        info!("Config file saved on '{}'", file.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    mod archive {
        use crate::prelude::ArtidArchive;
        use chrono::Utc;
        use std::fs;
        use tempfile;

        #[test]
        fn test_archive_load_from_valid() {
            let dir = tmpdir!();
            create_file!(
                tmppath!(dir, "artid.toml"),
                include_str!("../../../tests/files/single_folder.toml")
            );
            assert!(ArtidArchive::load_from(dir, "artid.toml").is_ok());
        }

        #[test]
        fn test_archive_load_with_snapshot() {
            let dir = tmpdir!();
            create_file!(
                tmppath!(dir, "artid.toml"),
                include_str!("../../../tests/files/single_folder_snapshot.toml"),
                rfc3339!(Utc::now())
            );
            assert!(ArtidArchive::load_from(dir, "artid.toml").is_ok());
        }

        #[test]
        fn test_archive_load() {
            let tmp = tmpdir!();
            fs::create_dir(tmppath!(tmp, ".artid")).expect("Unable to create folder");
            create_file!(
                tmppath!(tmp, ".artid/artid.toml"),
                include_str!("../../../tests/files/single_folder_origin_creation.toml"),
                tmppath!(tmp, "origin").display().to_string()
            );
            assert!(ArtidArchive::load(tmp.path()).is_ok());
        }

        #[test]
        fn test_archive_file_save_exists() {
            let dir = tmpdir!();
            assert!(create_file!(tmppath!(dir, "artid.toml")).exists());

            let archive = ArtidArchive::new(dir.path());
            assert!(archive.save_to("artid.toml").is_ok());
        }

        #[test]
        fn test_archive_file_save_unexistant() {
            let dir = tmpdir!();

            let archive = ArtidArchive::new(dir.path());
            assert!(archive.save_to("artid.toml").is_ok());
        }

        #[test]
        fn test_archive_save_to_format() {
            let tmp = tmpdir!();
            create_file!(
                tmppath!(tmp, "artid.toml"),
                include_str!("../../../tests/files/single_folder_origin_creation.toml"),
                tmppath!(tmp, "origin").display().to_string()
            );

            let archive =
                ArtidArchive::load_from(tmp.path(), "artid.toml").expect("Unable to load file");
            archive
                .save_to("artid2.toml")
                .expect("Unable to save the file");

            assert_eq!(
                toml::to_string_pretty(&archive.archive).expect("Cannot fail serialization"),
                read_file!(tmppath!(tmp, "artid2.toml")),
            );
        }

        #[test]
        fn test_archive_save() {
            let tmp = tmpdir!();
            fs::create_dir(tmppath!(tmp, ".artid")).expect("Unable to create folder");

            let archive = ArtidArchive::new(tmp.path());
            archive.save().expect("Unable to save");

            assert!(tmppath!(tmp, ".artid/artid.toml").exists());
        }
    }
}
