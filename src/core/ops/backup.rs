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

use super::core;
use super::core::filesystem::{FileSystem, Local, Route};
use super::core::model::{CopyAction, CopyModel, MultipleCopyModel};
use super::{Model, Operation, Operator};
use crate::config::archive::{FolderHistory, Link};
use crate::prelude::{ArtidArchive, ConfigFile, FileSystemFolder};

#[allow(missing_docs)]
pub type Action = CopyAction<Local, Local>;

#[allow(missing_docs)]
pub type Actions = core::model::Actions<Local, Local>;

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

/// Modifiers for the archive backup operation.
///
/// The default implementation for the options is equal to specifying all folders for the
/// snapshot
#[derive(Clone, Debug, Default)]
pub struct ArchiveOptions {
    folders: Option<Vec<String>>,
}

impl ArchiveOptions {
    /// Create the options selecting a set of folders for the snapshot
    pub fn with_folders(folders: Vec<String>) -> Self {
        let folders = Some(folders);
        Self { folders }
    }
}

/// Represents the backup operation. It's purpouse is to be the operation called for
/// <Type as Operator<Operation>>::modelate(...)
pub struct Backup;

impl Backup {
    fn with_previous(base: &Path, old: &Path, new: &Path) -> Result<Actions, io::Error> {
        use self::core::tree::{DirTree, Direction, FileType, Presence};

        let base = Local::new(base);
        let old = Local::new(old);
        let new = Local::new(new);

        let tree = DirTree::new(&base, &old)?;
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

    fn from_scratch(base: &Path, new: &Path) -> Result<Actions, io::Error> {
        use self::core::tree::{DirTree, FileType};

        let base = Local::new(base);
        let new = Local::new(new);

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

impl<'mo, P: AsRef<Path> + Debug> Operator<'mo, Backup> for ArtidArchive<P> {
    type Model = MultipleCopyModel<'mo, 'mo, Local, Local>;
    type Error = io::Error;
    type Options = ArchiveOptions;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        let stamp = Utc::now();
        let (root, history) = (&self.folder, &self.archive.history);
        let (folders, models): (Vec<String>, Vec<_>) = self
            .archive
            .config
            .folders
            .iter()
            .filter(|folder| match options.folders {
                Some(ref folders) => folders.iter().any(|name| folder.name == *name),
                None => true,
            })
            .map(|folder| {
                let link = folder.resolve(&root);
                let actions = create_actions(link, history.find(folder), stamp)?;
                Ok((folder.name.clone(), CopyModel::new(actions, || {})))
            })
            .try_fold(
                (vec![], vec![]),
                |(mut folders, mut models), result: Result<_, io::Error>| match result {
                    Ok((folder, model)) => {
                        folders.push(folder);
                        models.push(model);
                        Ok((folders, models))
                    }
                    Err(err) => Err(err),
                },
            )?;

        Ok(MultipleCopyModel::new(models, move || {
            self.archive.history.add_snapshot(stamp, folders)
        }))
    }
}

impl<'mo, P: AsRef<Path> + Debug> Operator<'mo, Backup> for ConfigFile<P> {
    type Model = MultipleCopyModel<'mo, 'mo, Local, Local>;
    type Error = io::Error;
    type Options = Options;

    fn modelate(&'mo mut self, options: Self::Options) -> Result<Self::Model, Self::Error> {
        let stamp = Utc::now();
        let dir = &self.dir;

        Ok(MultipleCopyModel::new(
            self.folders
                .contents
                .iter_mut()
                .map(|e| {
                    let folder = e.apply_root(&dir);

                    Ok(CopyModel::new(
                        actions(&folder, options, stamp)?,
                        move || {
                            folder.config.add_modified(stamp);
                        },
                    ))
                })
                .collect::<Result<_, io::Error>>()?,
            || {},
        ))
    }
}

impl<'mo, 'a: 'mo> Operator<'mo, Backup> for FileSystemFolder<'a> {
    type Model = CopyModel<'mo, Local, Local>;
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

fn create_actions(
    link: Link,
    history: FolderHistory<'_, '_>,
    stamp: DateTime<Utc>,
) -> Result<Actions, io::Error> {
    if let Some(modified) = history.find_last_sync() {
        let old = link.relative.join(rfc3339!(modified));
        let new = link.relative.join(rfc3339!(stamp));
        Backup::with_previous(&link.origin, &old, &new)
    } else {
        let relative = link.relative.join(rfc3339!(stamp));
        Backup::from_scratch(&link.origin, &relative)
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
    use super::{Backup, Model, Operator, Options};
    use crate::prelude::{FileSystemFolder, FolderConfig};

    macro_rules! filetree {
        ($var:ident, $join:expr, $push:expr) => {{
            let mut path = $var.path().join($join);

            path.push(rfc3339!($push));

            let tree: FileTree<_> = path.into();
            tree
        }};
    }

    #[test]
    fn test_folder_backup_single() {
        let origin = FileTree::generate();
        let root = FileTree::create();

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Backup);

        let mut backup = filetree!(
            root,
            "backup",
            folder
                .config
                .find_sync(0)
                .expect("The backup was not registered")
        );
        backup.copy_tree(&origin);
        backup.assert();
    }

    #[test]
    fn test_archive_backup_single() {
        let origin = FileTree::generate();
        let root = FileTree::create();

        let options = ArchiveOptions::default();
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        run!(archive, options, Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_folder_backup_double() {
        let origin = FileTree::generate();
        let root = FileTree::create();

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(0).unwrap());
        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(1).unwrap());
        backup.copy_tree(&origin);
        backup.transform("a.txt", FileKind::Symlink);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_backup_double() {
        let origin = FileTree::generate();
        let root = FileTree::create();

        let options = ArchiveOptions::default();
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.transform("a.txt", FileKind::Symlink);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_folder_backup_double_addition() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(0).unwrap());
        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.add_file("c.txt");
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(1).unwrap());
        backup.copy_tree(&origin);
        backup.transform("a.txt", FileKind::Symlink);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_backup_double_addition() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = ArchiveOptions::default();
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.add_file("c.txt");
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.transform("a.txt", FileKind::Symlink);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_folder_backup_double_modification() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(0).unwrap());
        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.modify("a.txt", "aaaa");
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(1).unwrap());
        backup.copy_tree(&origin);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_backup_double_modification() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = ArchiveOptions::default();
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.modify("a.txt", "aaaa");
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_folder_backup_double_remotion() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = Options::default();
        let mut config = FolderConfig::new("backup", origin.path());
        let mut folder = config.apply_root(root.path());
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(0).unwrap());
        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.remove("a.txt");
        run!(folder, options, Backup);

        let mut backup = filetree!(root, "backup", folder.config.find_sync(1).unwrap());
        backup.copy_tree(&origin);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }

    #[test]
    #[ignore]
    fn test_archive_backup_double_remotion() {
        let mut origin = FileTree::generate();
        let root = FileTree::create();

        let options = ArchiveOptions::default();
        let mut archive = ArtidArchive::new(root.path());
        archive.add_folder("backup", origin.path().display().to_string());
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.assert();

        thread::sleep(time::Duration::from_millis(2000));
        origin.remove("a.txt");
        run!(archive, options.clone(), Backup);

        let mut backup = filetree!(
            root,
            "backup",
            archive
                .archive
                .history
                .find(&archive.archive.config.folders[0])
                .find_last_sync()
                .expect("The backup was not registered")
        );

        backup.copy_tree(&origin);
        backup.transform("b.txt", FileKind::Symlink);
        backup.assert();
    }
}
