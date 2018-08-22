use failure::{Fail, ResultExt};
use logger::pathlight;
use std::cell::Ref;
use std::ffi::OsString;
use std::fs::{self, ReadDir};
use std::path::Path;
use {FsError, Result};

use super::{DirBranch, DirRoot, LinkedPoint, SyncOptions};

/// Used to handle errors in the sync process.
macro_rules! handle {
    ($warn:expr, $err:expr, $($msg:tt)*) => {
        if $warn {
            warn!($($msg)*);
            if cfg!(debug_assertions) {
                for cause in $err.causes() {
                    trace!("{}", cause);
                }
            }
        } else {
            fail!($err);
        }
    };
}

/// Used to give an object the ability to generate a 'branch' of itself. The generic type
/// T represents the type of branch that the object will generate. P represents the data needed
/// to generate the branch of the object.
///
/// This trait does two things:
///  - Generate a branch of the object through .branch()
///  - Return to the root point using the root method. This method should be called in the drop
///    implementation of T instead of calling it directly
pub(super) trait Branchable<'a, T: 'a, P> {
    fn branch(&'a self, branch: P) -> T;
    fn root(&self);
}

/// Used to give an object the ability to represent a link between two locations.
pub(super) trait Linkable<'a, T> {
    type Link: 'a;

    fn to_ref(&self) -> Ref<T>;
    fn link(&'a self) -> Self::Link;
}

/// Internal recursive function used to sync two trees by using branches. See the docs of
/// DirTree::sync to understand how this function works on a general level.
pub(super) fn sync<'a, T, O>(tree: &'a T, options: O) -> Result<()>
where
    T: 'a
        + for<'b> Branchable<'a, DirBranch<'a>, &'b OsString>
        + for<'b> Linkable<'b, DirRoot, Link = LinkedPoint<'b>>,
    O: Into<SyncOptions>,
{
    let mut options = options.into();
    check(tree.to_ref(), &mut options.clean)?;

    for entry in read_src(tree.to_ref())?.into_iter().filter_map(|e| e.ok()) {
        let branch = tree.branch(&entry.file_name());

        // done separatly to avoid RefCell issues
        let class = FileSystemType::from(&branch.to_ref().src);
        match class {
            FileSystemType::File => {
                if let Err(err) = branch.link().mirror(options.overwrite) {
                    handle!(
                        options.warn,
                        err,
                        "Unable to copy {}",
                        pathlight(&branch.to_ref().src)
                    );
                }
            }

            FileSystemType::Dir => {
                if let Err(err) = sync(&branch, options) {
                    handle!(
                        options.warn,
                        err,
                        "Unable to read {}",
                        pathlight(&branch.to_ref().src)
                    );
                }
            }

            FileSystemType::Other => {
                warn!("Unable to process {}", pathlight(&branch.to_ref().src));
            }
        }
    }

    if options.clean {
        clean(tree)?;
    }

    Ok(())
}

// The next three functions are simply wrappers for reducing the size of the sync
// function. Since there are RefCells involved be specially careful when modifying any
// part of these code

#[inline(always)]
fn check(tree: Ref<DirRoot>, clean: &mut bool) -> Result<()> {
    let (src, dst) = (pathlight(&tree.src), pathlight(&tree.dst));
    debug!("Syncing {} with {}", dst, src);

    if !tree.dst.is_dir() {
        fs::create_dir_all(&tree.dst).context(FsError::OpenFile((&tree.dst).into()))?;
        *clean = false; // useless to perform cleaning on a new dir.
    }

    Ok(())
}

#[inline(always)]
fn read_src(tree: Ref<DirRoot>) -> Result<ReadDir> {
    Ok(fs::read_dir(&tree.src).context(FsError::ReadFile((&tree.src).into()))?)
}

fn read_dst(tree: Ref<DirRoot>) -> Result<ReadDir> {
    Ok(fs::read_dir(&tree.dst).context(FsError::ReadFile((&tree.src).into()))?)
}

/// Internal recursive function used to clean the backup directory of garbage files.
fn clean<'a, T>(tree: &'a T) -> Result<()>
where
    T: 'a
        + for<'b> Branchable<'a, DirBranch<'a>, &'b OsString>
        + for<'b> Linkable<'b, DirRoot, Link = LinkedPoint<'b>>,
{
    for entry in read_dst(tree.to_ref())?.filter_map(|e| e.ok()) {
        let branch = tree.branch(&entry.file_name());

        if !branch.to_ref().src.exists() {
            debug!(
                "Unnexistant {}, removing {}",
                pathlight(&branch.to_ref().src),
                pathlight(&branch.to_ref().dst)
            );

            if branch.to_ref().dst.is_dir() {
                fs::remove_dir_all(&branch.to_ref().dst)
                    .context(FsError::DeleteFile((&branch.to_ref().dst).into()))?;
            } else {
                fs::remove_file(&branch.to_ref().dst)
                    .context(FsError::DeleteFile((&branch.to_ref().dst).into()))?;
            }
        }
    }

    Ok(())
}

/// Represents the different types a path can take on the file system. It is just a convenience
/// enum for using a match instead of an if-else tree.
#[derive(Debug, PartialEq)]
enum FileSystemType {
    File,
    Dir,
    Other,
}

impl<P: AsRef<Path>> From<P> for FileSystemType {
    fn from(path: P) -> Self {
        let path = path.as_ref();
        if path.is_file() {
            FileSystemType::File
        } else if path.is_dir() {
            FileSystemType::Dir
        } else {
            FileSystemType::Other
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate tempfile;

    use super::{check, DirRoot, FileSystemType};
    use std::cell::RefCell;
    use std::fs::File;

    #[test]
    fn test_system_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(FileSystemType::from(dir.path()), FileSystemType::Dir);
    }

    #[test]
    fn test_system_file() {
        let dir = tempfile::tempdir().unwrap();
        let _file = File::create(dir.path().join("a.txt"));
        assert_eq!(
            FileSystemType::from(dir.path().join("a.txt")),
            FileSystemType::File
        );
    }

    #[test]
    fn test_check_creation() {
        let src = tempfile::tempdir().unwrap();
        let dst = src.path().join("asd");
        let mut clean = true;

        let root = RefCell::new(DirRoot::new(src.path().into(), dst.clone()));
        check(root.borrow(), &mut clean).unwrap();

        assert!(dst.exists(), "Directory was not created");
        assert_eq!(clean, false);
    }

    #[test]
    fn test_check_not_creation() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let mut clean = true;

        let root = RefCell::new(DirRoot::new(src.path().into(), dst.path().into()));
        check(root.borrow(), &mut clean).unwrap();

        assert_eq!(clean, true);
    }

}
