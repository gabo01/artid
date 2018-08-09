use std::path::Path;
use super::{ConfigFile, Result};

pub struct Backup<T: AsRef<Path>> {
    path: T
}

impl<T: AsRef<Path>> Backup<T> {
    pub fn new(path: T) -> Self {
        Backup {path}
    }

    pub fn execute(&self) -> Result<()> {
        let _file = ConfigFile::load(&self.path)?;
        Ok(())
    }
}
