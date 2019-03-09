use chrono::{offset::Utc, DateTime};
use env_path::EnvPath;
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::path::Path;

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Archive {
    #[serde(rename = "system")]
    pub(crate) config: Config,
    pub(crate) history: History,
}

impl Archive {
    pub(crate) fn add_folder<P, O>(&mut self, path: P, origin: O)
    where
        P: Into<String>,
        O: Into<String>,
    {
        self.config
            .folders
            .push(Folder::new(path, origin, self.config.hasher))
    }

    pub(crate) fn add_snapshot(&mut self, timestamp: DateTime<Utc>) {
        self.history.add_snapshot(
            timestamp,
            self.config
                .folders
                .iter()
                .map(|folder| folder.name.to_owned())
                .collect(),
        )
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
pub struct Config {
    /// determines the type of hash algorithm to use
    hasher: Hasher,
    #[serde(rename = "folder")]
    folders: Folders,
}

#[derive(Copy, Clone, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum Hasher {
    #[serde(rename = "sha-3")]
    Sha3,
}

#[derive(Debug)]
pub struct Folders {
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
pub struct History {
    #[serde(rename = "snapshot")]
    snapshots: Snapshots,
}

impl History {
    pub fn add_snapshot(&mut self, timestamp: DateTime<Utc>, folders: Vec<String>) {
        self.snapshots.push(Snapshot::new(timestamp, folders));
    }
}

#[derive(Debug)]
pub struct Snapshots {
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

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Snapshot {
    timestamp: DateTime<Utc>,
    /// List of folders modified
    folders: Vec<String>,
}

impl Snapshot {
    pub fn new(timestamp: DateTime<Utc>, folders: Vec<String>) -> Self {
        Self { timestamp, folders }
    }
}

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
}
