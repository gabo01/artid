//! Holds all the logic for performing a restore operation
//!
//! The easiest way to use this module is through the global helper 'restore'. The restore
//! function will return the associated restore model for the given operator, meaning that
//! the actual model returned may vary based on the operator.

use chrono::{DateTime, Utc};
use log::{debug, info, log};
use std::fmt::{self, Debug, Display};
use std::io;
use std::path::Path;

use super::core::filesystem::{FileSystem, Local, Route};
use super::core::model::{CopyAction, CopyModel, MultipleCopyModel};
use super::core::{self, Error, ErrorKind};
use super::{Model, Operation, Operator};
use crate::config::archive::Link;
use crate::prelude::ArtidArchive;

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

/// Options to modify behaviour of the restore operation in the archive
#[derive(Clone, Debug)]
pub struct ArchiveOptions {
    overwrite: bool,
    snapshot: Option<DateTime<Utc>>,
    folders: Option<Vec<String>>,
}

impl ArchiveOptions {
    /// Create a new set of options specifying to overwrite or not the files in the destination.
    ///
    /// By default, it selects to restore the last snapshot and all the folders.
    pub fn new(overwrite: bool) -> Self {
        Self {
            overwrite,
            snapshot: None,
            folders: None,
        }
    }

    /// Allows to specify a previous snapshot to restore. In case that the snapshot does
    /// no exist, the restore will return an error.
    pub fn with_snapshot(mut self, snapshot: DateTime<Utc>) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Select the folders to restore, the folders must be referenced by their respective
    /// id's.
    pub fn with_folders(mut self, folders: Vec<String>) -> Self {
        self.folders = Some(folders);
        self
    }
}

/// Represents the restore operation. It's purpouse is to be the operation called for
/// <Type as Operator<Operation>>::modelate(...)
pub struct Restore;

impl Restore {
    fn from_point(restore: &Path, backup: &Path, overwrite: bool) -> Result<Actions, Error> {
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

impl<'mo, P: AsRef<Path> + Debug> Operator<'mo, Restore> for ArtidArchive<P> {
    type Model = MultipleCopyModel<'mo, 'mo, Local, Local>;
    type Error = Error;
    type Options = ArchiveOptions;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        let root = &self.folder;
        let image = match options.snapshot {
            Some(timestamp) => self
                .archive
                .history
                .snapshot_with(timestamp)
                .map(|snapshot| self.archive.history.pin(snapshot)),
            None => self
                .archive
                .history
                .get_last_snapshot()
                .map(|snapshot| self.archive.history.pin(snapshot)),
        };

        if let Some(image) = image {
            Ok(MultipleCopyModel::new(
                self.archive
                    .config
                    .folders
                    .iter()
                    .filter(|folder| match options.folders {
                        Some(ref folders) => folders.iter().any(|name| folder.name == *name),
                        None => true,
                    })
                    .filter_map(|folder| {
                        let link = folder.resolve(&root);
                        let timestamp = match image.last_timestamp(&folder) {
                            Some(timestamp) => timestamp,
                            None => return None,
                        };

                        let actions = Restore::from_point(
                            &link.origin,
                            &link.relative.join(rfc3339!(timestamp)),
                            options.overwrite,
                        );

                        Some(actions.map(|actions| CopyModel::new(actions, || {})))
                    })
                    .collect::<Result<_, Self::Error>>()?,
                || {},
            ))
        } else {
            Err(Error::new(ErrorKind::PointNotExists))
        }
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
    use super::{ArchiveOptions, ArtidArchive};
    use super::{Model, Operator, Restore};

    macro_rules! backup {
        ($root:ident, $stamp:ident, $generate:expr) => {
            backup!($root, $stamp, $generate, "backup")
        };

        ($root:ident, $stamp:ident, $generate:expr, $name:expr) => {{
            let format = format!("{}/{}", $name, rfc3339!($stamp));
            let path = tmppath!($root, format);
            if $generate {
                FileTree::generate_from(path)
            } else {
                FileTree::new(path)
            }
        }};
    }

    #[test]
    fn test_archive_restore_single() {
        let mut origin = FileTree::create();
        let (root, stamp) = (tmpdir!(), Utc::now());
        let backup = backup!(root, stamp, true);

        let options = ArchiveOptions::new(false);
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        archive
            .archive
            .history
            .add_snapshot(stamp, vec![archive.archive.config.folders[0].name.clone()]);
        run!(archive, options, Restore);

        origin.copy_tree(&backup);
        origin.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_restore_with_symlinks() {
        let mut origin = FileTree::create();
        let (root, stamp) = (tmpdir!(), Utc::now());
        let backup = backup!(root, stamp, true);

        thread::sleep(time::Duration::from_millis(2000));
        let stamp_new = Utc::now();
        let mut backup_second = backup!(root, stamp_new, false);
        backup_second.add_root();
        backup_second.add_symlink("a.txt", backup.path().join("a.txt"));
        backup_second.add_symlink("b.txt", backup.path().join("b.txt"));

        let options = ArchiveOptions::new(false);
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        archive.archive.history.add_snapshot(
            stamp_new,
            vec![archive.archive.config.folders[0].name.clone()],
        );
        run!(archive, options, Restore);

        origin.copy_tree(&backup);
        origin.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_restore_multiple_folders_with_previous() {
        let mut backup_origin = FileTree::create();
        let mut side_origin = FileTree::create();
        let (root, stamp) = (tmpdir!(), Utc::now());
        let backup = backup!(root, stamp, true);
        let side = backup!(root, stamp, true, "side");

        thread::sleep(time::Duration::from_millis(2000));
        let stamp_new = Utc::now();
        let mut backup_second = backup!(root, stamp_new, false);
        backup_second.add_root();
        backup_second.add_symlink("a.txt", backup.path().join("a.txt"));
        backup_second.add_symlink("b.txt", backup.path().join("b.txt"));

        let options = ArchiveOptions::new(false);
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", backup_origin.path().display().to_string());
        archive.add_folder("side", side_origin.path().display().to_string());
        archive.archive.history.add_snapshot(
            stamp,
            vec![
                archive.archive.config.folders[0].name.clone(),
                archive.archive.config.folders[1].name.clone(),
            ],
        );
        archive.archive.history.add_snapshot(
            stamp_new,
            vec![archive.archive.config.folders[0].name.clone()],
        );
        run!(archive, options, Restore);

        backup_origin.copy_tree(&backup);
        backup_origin.assert();
        side_origin.copy_tree(&side);
        side_origin.assert();
    }
}
