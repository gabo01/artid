//! Holds all the logic for performing a restore operation
//!
//! The easiest way to use this module is through the global helper 'restore'. The restore
//! function will return the associated restore model for the given operator, meaning that
//! the actual model returned may vary based on the operator. The current operators are:
//!
//! - ConfigFile<P>: will return a model to execute a new restore for every registered folder
//!   and fail if any of the singular models fails to build.
//! - FileSystemFolder: will return a model to execute a restore on the singular folder.
//!   calling this model for every folder is equivalent to performing a mass restore

use chrono::{DateTime, Utc};
use failure::{Backtrace, Context, Fail, ResultExt};
use log::{debug, info, log};
use std::fmt::{self, Debug, Display};
use std::io;
use std::path::Path;

use super::core;
use super::core::filesystem::{FileSystem, Local, Route};
use super::core::model::{CopyAction, CopyModel, MultipleCopyModel};
use super::{Model, Operation, Operator};
use crate::prelude::{ConfigFile, FileSystemFolder};

#[allow(missing_docs)]
pub type Action = CopyAction<Local, Local>;

#[allow(missing_docs)]
pub type Actions = core::model::Actions<Local, Local>;

/// This function is responsible for making the restore model for the given operator
pub fn restore<'a, O: Operator<'a, Restore>>(
    operator: &'a mut O,
    options: O::Options,
) -> Result<O::Model, O::Error> {
    operator.modelate(options)
}

/// Underlying kind of error responsible for failing the building of the restore model
#[derive(Debug, Fail)]
pub enum BuildErrorKind {
    #[allow(missing_docs)]
    #[fail(display = "The folder does not have the requested restore point")]
    PointNotExists,
    #[allow(missing_docs)]
    #[fail(display = "Error while building the model from disk, see the cause for details")]
    FsError,
}

/// Error that can be returned when building a restore model
#[derive(Debug)]
pub struct BuildError {
    inner: Context<BuildErrorKind>,
}

impl Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Fail for BuildError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl From<BuildErrorKind> for BuildError {
    fn from(kind: BuildErrorKind) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<BuildErrorKind>> for BuildError {
    fn from(inner: Context<BuildErrorKind>) -> Self {
        Self { inner }
    }
}

/// Modifier options for the restore operation
#[derive(Copy, Clone, Debug)]
pub struct Options {
    overwrite: bool,
    point: Option<usize>,
}

impl Options {
    #[allow(missing_docs)]
    pub fn new(overwrite: bool) -> Self {
        Self {
            overwrite,
            point: None,
        }
    }

    #[allow(missing_docs)]
    pub fn with_point(overwrite: bool, point: usize) -> Self {
        Self {
            overwrite,
            point: Some(point),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            overwrite: false,
            point: None,
        }
    }
}

/// Represents the restore operation. It's purpouse is to be the operation called for
/// <Type as Operator<Operation>>::modelate(...)
pub struct Restore;

impl Restore {
    fn from_point(restore: &Path, backup: &Path, overwrite: bool) -> Result<Actions, io::Error> {
        use self::core::tree::{DirTree, FileType, Presence};

        let restore = Local::new(restore);
        let backup = Local::new(backup);

        let tree = DirTree::new(&restore, &backup)?;
        Ok(tree
            .iter()
            .filter(|e| {
                e.presence() == Presence::Dst
                    || overwrite && e.presence() == Presence::Both && e.kind() != FileType::Dir
            })
            .map(|e| {
                if e.kind() == FileType::Dir && e.presence() == Presence::Dst {
                    CopyAction::CreateDir {
                        target: restore.join(e.path()),
                    }
                } else {
                    CopyAction::CopyFile {
                        src: backup.join(e.path()),
                        dst: restore.join(e.path()),
                    }
                }
            })
            .collect())
    }
}

impl Operation for Restore {}

impl<'mo, P: AsRef<Path> + Debug> Operator<'mo, Restore> for ConfigFile<P> {
    type Model = MultipleCopyModel<'mo, Local, Local>;
    type Error = BuildError;
    type Options = Options;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        let dir = &self.dir;

        Ok(MultipleCopyModel::new(
            self.folders
                .iter_mut()
                .map(|e| {
                    let folder = e.apply_root(&dir);
                    Ok(CopyModel::new(actions(&folder, options)?, || {}))
                })
                .collect::<Result<_, BuildError>>()?,
        ))
    }
}

impl<'mo> Operator<'mo, Restore> for FileSystemFolder<'mo> {
    type Model = CopyModel<'mo, Local, Local>;
    type Error = BuildError;
    type Options = Options;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        Ok(CopyModel::new(actions(&self, options)?, || {}))
    }
}

fn actions(folder: &FileSystemFolder, options: Options) -> Result<Actions, BuildError> {
    if folder.config.has_sync() {
        let modified = match options.point {
            Some(point) => folder.config.find_sync(point),
            None => folder.config.find_last_sync(),
        };

        if let Some(modified) = modified {
            debug!("Starting restore of: {}", folder.link.relative.display());
            let relative = folder.link.relative.join(rfc3339!(modified));

            Ok(
                Restore::from_point(&folder.link.origin, &relative, options.overwrite)
                    .context(BuildErrorKind::FsError)?,
            )
        } else {
            Err(BuildErrorKind::PointNotExists)?
        }
    } else {
        info!("Restore not needed for {}", folder.link.relative.display());
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time;
    use tempfile::TempDir;

    use super::super::test_helpers::{FileKind, FileTree};
    use super::{Model, Operator, Options, Restore};
    use crate::prelude::{FileSystemFolder, FolderConfig};

    macro_rules! backup {
        ($root:ident, $stamp:ident, $generate:expr) => {{
            let format = format!("backup/{}", rfc3339!($stamp));
            let path = tmppath!($root, format);
            if $generate {
                FileTree::generate_from(path)
            } else {
                FileTree::new(path)
            }
        }};
    }

    #[test]
    fn test_folder_restore_single() {
        let mut origin = FileTree::create();
        let (root, stamp) = (tmpdir!(), Utc::now());
        let backup = backup!(root, stamp, true);

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        config.add_modified(stamp); // in order to detect the backup
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Restore);

        origin.copy_tree(&backup);
        origin.assert();
    }

    #[test]
    #[ignore]
    fn test_folder_restore_with_symlinks() {
        let mut origin = FileTree::create();
        let (root, stamp) = (tmpdir!(), Utc::now());
        let backup = backup!(root, stamp, true);

        thread::sleep(time::Duration::from_millis(2000));
        let stamp_new = Utc::now();
        let mut backup_second = backup!(root, stamp_new, false);
        backup_second.add_root();
        backup_second.add_symlink("a.txt", backup.path().join("a.txt"));
        backup_second.add_symlink("b.txt", backup.path().join("b.txt"));

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        config.add_modified(stamp_new); // in order to detect the last backup
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Restore);

        origin.copy_tree(&backup);
        origin.assert();
    }
}
