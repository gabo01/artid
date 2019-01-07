//! Holds all the logic for performing a backup operation
//!
//! The easiest way to use this module is through the global helper 'backup'. The backup
//! function will return the associated backup model for the given operator, meaning that
//! the actual model returned may vary based on the operator. The current operators are:
//!
//! - ConfigFile<P>: will return a model to execute a new backup for every registered folder
//!   and fail if any of the singular models fails to build.
//! - FileSystemFolder: will return a model to execute a backup on the singular folder. In
//!   case of wanting to perform a mass backup is better to use the global backup directly as
//!   it will apply the same stamp to all the folders.

use chrono::{DateTime, Utc};
use failure::Fail;
use std::fmt::Debug;
use std::io;
use std::path::Path;

use super::{
    core::{self, Actions, CopyAction, CopyModel, MultipleCopyModel},
    Model, Operation, Operator,
};
use prelude::{ConfigFile, FileSystemFolder};

/// This function is responsible for making the backup model for the given operator
pub fn backup<'a, O: Operator<'a, Backup>>(
    operator: &'a mut O,
    options: O::Options,
) -> Result<O::Model, O::Error> {
    operator.modelate(options)
}

/// Modifiers for the backup operation
#[derive(Copy, Clone, Debug)]
pub struct Options;

impl Default for Options {
    fn default() -> Self {
        Self {}
    }
}

/// Represents the backup operation. It's purpouse is to be the operation called for
/// <Type as Operator<Operation>>::modelate(...)
pub struct Backup;

impl Backup {
    fn with_previous(base: &Path, old: &Path, new: &Path) -> Result<Actions, io::Error> {
        use self::core::{DirTree, Direction, Presence};

        let tree = DirTree::new(base, old)?;
        Ok(tree
            .iter()
            .filter(|e| e.presence() != Presence::Dst)
            .map(|e| {
                if e.kind() == core::FileType::Dir {
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

    fn from_scratch(base: &Path, new: &Path) -> Result<Actions, io::Error> {
        use self::core::{DirTree, FileType};

        let tree = DirTree::new(&base, &new)?;
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

impl Operation for Backup {}

impl<'mo, P: AsRef<Path> + Debug> Operator<'mo, Backup> for ConfigFile<P> {
    type Model = MultipleCopyModel<'mo>;
    type Error = io::Error;
    type Options = Options;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        let stamp = Utc::now();
        let dir = &self.dir;

        Ok(MultipleCopyModel::new(
            self.folders
                .iter_mut()
                .map(|e| {
                    let mut folder = e.apply_root(&dir);

                    Ok(CopyModel::new(
                        actions(&folder, options, stamp)?,
                        move || {
                            folder.config.add_modified(stamp);
                        },
                    ))
                })
                .collect::<Result<_, io::Error>>()?,
        ))
    }
}

impl<'mo, 'a: 'mo> Operator<'mo, Backup> for FileSystemFolder<'a> {
    type Model = CopyModel<'mo>;
    type Error = io::Error;
    type Options = Options;

    fn modelate(&'mo mut self, options: Options) -> Result<Self::Model, Self::Error> {
        let stamp = Utc::now();
        let actions = actions(&self, options, stamp)?;

        Ok(CopyModel::new(actions, move || {
            self.config.add_modified(stamp);
        }))
    }
}

fn actions(
    folder: &FileSystemFolder<'_>,
    _options: Options,
    stamp: DateTime<Utc>,
) -> Result<Actions, io::Error> {
    if let Some(modified) = folder.config.find_last_sync() {
        let old = folder.link.relative.join(rfc3339!(modified));
        let new = folder.link.relative.join(rfc3339!(stamp));
        Backup::with_previous(&folder.link.origin, &old, &new)
    } else {
        let relative = folder.link.relative.join(rfc3339!(stamp));
        Backup::from_scratch(&folder.link.origin, &relative)
    }
}
