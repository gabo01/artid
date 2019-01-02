//! Contains the implementations related to the backup operation

use failure::ResultExt;
use std::path::Path;

use sync::{CopyAction, CopyModel, DirTree, Direction, FileType, Presence};

use super::errors::{OperativeError, OperativeErrorType};

/// Modifier options for the backup action on ConfigFile. Check the properties to see which
/// behaviour they control
#[derive(Debug, Copy, Clone)]
pub struct BackupOptions {
    /// Controls if the model should be ran or not. In case the model does not run, the
    /// intended actions will be logged into the screen
    pub(crate) run: bool,
}

impl BackupOptions {
    /// Creates a new set of options for the backup operation.
    pub fn new(run: bool) -> Self {
        Self { run }
    }
}

pub struct Backup;

impl Backup {
    pub fn with_previous(base: &Path, old: &Path, new: &Path) -> Result<CopyModel, OperativeError> {
        let tree = DirTree::new(base, old).context(OperativeErrorType::Scan)?;
        Ok(tree
            .iter()
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
            })
            .collect())
    }

    pub fn from_scratch(base: &Path, new: &Path) -> Result<CopyModel, OperativeError> {
        let tree = DirTree::new(&base, &new).context(OperativeErrorType::Scan)?;
        Ok(tree
            .iter()
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
            })
            .collect())
    }
}
