//! Contains all the elements required for the new archive implementation

use chrono::{offset::Utc, DateTime};
use env_path::EnvPath;
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct Archive {
    #[serde(rename = "system")]
    pub config: Config,
    pub history: History,
}

impl Archive {
    pub fn add_folder<P, O>(&mut self, path: P, origin: O)
    where
        P: Into<String>,
        O: Into<String>,
    {
        self.config
            .folders
            .push(Folder::new(path, origin, self.config.hasher))
    }
}

impl Default for Archive {
    fn default() -> Self {
        Self {
            config: Config {
                hasher: Hasher::Sha3,
                folders: Folders { inner: vec![] },
            },
            history: History {
                snapshots: Snapshots { inner: vec![] },
            },
        }
    }
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct Config {
    /// determines the type of hash algorithm to use
    hasher: Hasher,
    #[serde(rename = "folder")]
    pub folders: Folders,
}

/// Hasher algorithm to use for the archive operations
#[derive(Copy, Clone, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum Hasher {
    #[allow(missing_docs)]
    #[serde(rename = "sha-3")]
    Sha3,
}

#[derive(Debug)]
pub(crate) struct Folders {
    inner: Vec<Folder>,
}

impl Deref for Folders {
    type Target = Vec<Folder>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Folders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Serialize for Folders {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Folders {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            inner: <Vec<Folder> as Deserialize<'de>>::deserialize(deserializer)?,
        })
    }
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct History {
    #[serde(rename = "snapshot")]
    snapshots: Snapshots,
}

impl History {
    pub fn add_snapshot(&mut self, timestamp: DateTime<Utc>, folders: Vec<String>) {
        self.snapshots.push(Snapshot::new(timestamp, folders));
    }

    pub fn find<'a, 'b>(&'a self, folder: &'b Folder) -> FolderHistory<'a, 'b> {
        FolderHistory::new(self, &folder.name)
    }

    pub fn get_last_snapshot(&self) -> Option<Snapshot> {
        self.snapshots.last().map(ToOwned::to_owned)
    }

    pub fn snapshot_with(&self, stamp: DateTime<Utc>) -> Option<Snapshot> {
        self.snapshots
            .iter()
            .find(|snapshot| snapshot.timestamp == stamp)
            .map(ToOwned::to_owned)
    }
}

#[derive(Debug)]
pub(crate) struct Snapshots {
    inner: Vec<Snapshot>,
}

impl Deref for Snapshots {
    type Target = Vec<Snapshot>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Snapshots {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Serialize for Snapshots {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Snapshots {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            inner: <Vec<Snapshot> as Deserialize<'de>>::deserialize(deserializer)?,
        })
    }
}

/// Represents a snapshot taken and inserted to the archive
#[derive(Clone, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Snapshot {
    timestamp: DateTime<Utc>,
    /// List of folders modified
    folders: Vec<String>,
}

impl Snapshot {
    #[allow(missing_docs)]
    pub fn new(timestamp: DateTime<Utc>, folders: Vec<String>) -> Self {
        Self { timestamp, folders }
    }
}

/// Represents the structure of a folder inside the archive configuration file
///
/// This structure consists in a link between an origin (absolute path) and a
/// path relative to a specified root.
///
/// The id is an unique identifier used for the folder to allow changes to either
/// the elemets of the link.
#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Folder {
    /// Link path. If thinked as a link, this is where the symbolic link is
    pub(crate) path: EnvPath,
    /// Path of origin. If thinked as a link, this is the place the link points to
    pub(crate) origin: EnvPath,
    /// A hash to uniquely identify this folder even if the path changes
    #[serde(rename = "id")]
    pub(crate) name: String,
}

impl Folder {
    pub(crate) fn new<P, O>(path: P, origin: O, hasher: Hasher) -> Self
    where
        P: Into<String>,
        O: Into<String>,
    {
        let path = path.into();

        Self {
            path: EnvPath::new(path.clone()),
            origin: EnvPath::new(origin),
            name: match hasher {
                Hasher::Sha3 => {
                    use sha3::{Digest, Sha3_256};
                    let hash =
                        Sha3_256::digest(format!("{} + {}", rfc3339!(Utc::now()), path).as_bytes());

                    format!("{:x}", hash)
                }
            },
        }
    }

    pub(crate) fn resolve<P: AsRef<Path>>(&self, root: P) -> Link {
        Link {
            relative: root.as_ref().join(self.path.as_ref()),
            origin: self.origin.as_ref().into(),
        }
    }
}

pub(crate) struct Link {
    pub relative: PathBuf,
    pub origin: PathBuf,
}

pub(crate) struct FolderHistory<'a, 'b> {
    history: &'a History,
    folder: &'b str,
}

impl<'a, 'b> FolderHistory<'a, 'b> {
    pub fn new(history: &'a History, folder: &'b str) -> Self {
        Self { history, folder }
    }

    pub fn find_last_sync(&self) -> Option<DateTime<Utc>> {
        if self.history.snapshots.is_empty() {
            return None;
        }

        self.history
            .snapshots
            .iter()
            .rev()
            .find(|snapshot| snapshot.folders.iter().any(|folder| folder == self.folder))
            .map(|snapshot| snapshot.timestamp)
    }
}
