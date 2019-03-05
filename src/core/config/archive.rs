use chrono::{offset::Utc, DateTime};
use env_path::EnvPath;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Archive {
    #[serde(rename = "system")]
    pub(crate) config: Config,
    pub(crate) history: History,
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Config {
    /// determines the type of hash algorithm to use
    hasher: Hasher,
    #[serde(rename = "folder")]
    folders: Folders,
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum Hasher {
    #[serde(rename = "sha-3")]
    Sha3,
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
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

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct History {
    #[serde(rename = "snapshot")]
    snapshots: Snapshots,
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
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

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Snapshot {
    timestamp: DateTime<Utc>,
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
