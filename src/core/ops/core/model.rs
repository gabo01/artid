//! Contains the details needed for modeling most of artid's operations
//!
//! The main element of this module is the CopyModel that is composed by a
//! set of CopyActions and a callback to be executed after successfully running a model.

use std::fmt::Debug;
use std::fs;
use std::iter::FromIterator;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::path::PathBuf;

use super::super::Model;
use super::filesystem::{FileKind, FileSystem, Local, Metadata, Route};
use crate::{debug_closure::Debuggable, fn_box::FnBox};

#[allow(missing_docs)]
pub type Action<S, D> = CopyAction<S, D>;

/// An alias for a list of actions
pub type Actions<S, D> = Vec<CopyAction<S, D>>;

/// Represents the actions to perform in order to execute a full copy model for a specific operation.
/// These operations can be, but are not limited to, backup and restore. The different variants
/// represent the different operations an action can take. It is expected that these actions are
/// determined based on a comparison tree of the two locations which should have been previously
/// built.
#[derive(Debug)]
pub enum CopyAction<S, D>
where
    S: FileSystem,
    D: FileSystem,
{
    /// Creates a directory on the target location, this is expected to be done if a tree node
    /// is present in one location but not in the other. This action creates the target dir and
    /// any path ancestor not present already on the file system.
    CreateDir {
        #[allow(missing_docs)]
        target: D,
    },
    /// Performs a full copy of the file from src to dst, a thing to notice is that in complex
    /// operations, src and dst may not exactly match the result of taking src and dst + the node
    /// path from the tree.
    CopyFile {
        #[allow(missing_docs)]
        src: S,
        #[allow(missing_docs)]
        dst: D,
    },
    /// Creates a symlink on dst that points to src. As said on the CopyFile docs, src and dst may
    /// not exactly match, but are supposed to be derived from, the src and dst of the comparison
    /// tree.
    CopyLink {
        #[allow(missing_docs)]
        src: S,
        #[allow(missing_docs)]
        dst: D,
    },
}

/// Copy model for a specific operation. Take an operation such as a backup, you can describe that
/// operation with a series of actions such as: create dir a, copy file b to c. These model is
/// esentially a list of actions to perform in order to say an operation has been done and a
/// function to call after the operation is completed.
#[derive(Debug)]
pub struct CopyModel<'a, S, D>
where
    S: FileSystem,
    D: FileSystem,
{
    actions: Vec<CopyAction<S, D>>,
    cleaner: Debuggable<FnBox + 'a>,
}

impl<'a, S, D> CopyModel<'a, S, D>
where
    S: FileSystem,
    D: FileSystem,
{
    #[allow(missing_docs)]
    pub fn new<C: FnOnce() + 'a>(actions: Vec<CopyAction<S, D>>, cleaner: C) -> Self {
        Self {
            actions,
            cleaner: closure!(cleaner),
        }
    }
}

impl<'a, S, D> Default for CopyModel<'a, S, D>
where
    S: FileSystem + PartialEq<D>,
    D: FileSystem,
{
    fn default() -> Self {
        Self {
            actions: vec![],
            cleaner: closure!(|| {}),
        }
    }
}

impl<'a, S, D> Model for CopyModel<'a, S, D>
where
    S: FileSystem + PartialEq<D>,
    D: FileSystem,
{
    type Action = CopyAction<S, D>;
    type Error = ::std::io::Error;

    fn run(self) -> Result<(), Self::Error> {
        for action in &self.actions {
            apply(action)?;
        }

        self.cleaner.value.call_box();
        Ok(())
    }

    fn log<L: for<'b> Fn(&'b Self::Action)>(&self, logger: &L) {
        self.actions.iter().for_each(|e| logger(e));
    }

    fn log_run<L>(self, logger: &L) -> Result<(), Self::Error>
    where
        L: for<'b> Fn(&'b Self::Action),
    {
        for action in &self.actions {
            apply(action)?;
            logger(action);
        }

        Ok(())
    }
}

/// A set of individual models that are operated together
pub struct MultipleCopyModel<'a, S, D>
where
    S: FileSystem,
    D: FileSystem,
{
    models: Vec<CopyModel<'a, S, D>>,
}

impl<'a, S, D> MultipleCopyModel<'a, S, D>
where
    S: FileSystem,
    D: FileSystem,
{
    #[allow(missing_docs)]
    pub fn new(models: Vec<CopyModel<'a, S, D>>) -> Self {
        Self { models }
    }
}

impl<'a, S, D> Model for MultipleCopyModel<'a, S, D>
where
    S: FileSystem + PartialEq<D>,
    D: FileSystem,
{
    type Action = <CopyModel<'a, S, D> as Model>::Action;
    type Error = <CopyModel<'a, S, D> as Model>::Error;

    fn run(self) -> Result<(), Self::Error> {
        for model in self.models {
            model.run()?;
        }

        Ok(())
    }

    fn log<L: for<'b> Fn(&'b Self::Action)>(&self, logger: &L) {
        for model in &self.models {
            model.log(logger);
        }
    }

    fn log_run<L>(self, logger: &L) -> Result<(), Self::Error>
    where
        L: for<'b> Fn(&'b Self::Action),
    {
        for model in self.models {
            model.log_run(logger)?;
        }

        Ok(())
    }
}

fn apply<S, D>(action: &CopyAction<S, D>) -> ::std::io::Result<()>
where
    S: FileSystem + PartialEq<D>,
    D: FileSystem,
{
    match action {
        CopyAction::CreateDir { ref target } => {
            if !target.exists() {
                target.create_dir_all()?;
            }
        }

        CopyAction::CopyFile { ref src, ref dst } => {
            if let Ok(metadata) = dst.symlink_metadata() {
                if metadata.file_type().is_symlink() {
                    dst.remove_file()?;
                }
            }

            src.copy_to(dst)?;
        }

        CopyAction::CopyLink { ref src, ref dst } => {
            src.symlink_to(dst)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CopyAction, CopyModel};
    use super::{FileSystem, Local, Route};

    mod copy_model {
        use super::{CopyAction, CopyModel};
        use super::{FileSystem, Local, Route};
        use crate::ops::Model;
        use std::fs::File;
        use tempfile;

        #[test]
        fn test_create_dir() {
            let dir = tmpdir!();
            let action = CopyAction::CreateDir {
                target: Local::new(dir.path().join("asd")),
            };

            let model: CopyModel<'_, Local, Local> = CopyModel::new(vec![action], || {});
            model.run().expect("Unable to execute model");
            assert!(tmppath!(dir, "asd").exists());
            assert!(tmppath!(dir, "asd").is_dir());
        }

        #[test]
        fn test_create_nested_dir() {
            let dir = tmpdir!();
            let action = CopyAction::CreateDir {
                target: Local::new(dir.path().join("asd/as")),
            };

            let model: CopyModel<'_, Local, Local> = CopyModel::new(vec![action], || {});
            model.run().expect("Unable to execute model");
            assert!(tmppath!(dir, "asd/as").exists());
            assert!(tmppath!(dir, "asd/as").is_dir());
        }

        #[test]
        fn test_create_file() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CopyFile {
                    src: Local::new(a_path.clone()),
                    dst: Local::new(tmppath!(dst, "a.txt")),
                },
                CopyAction::CopyFile {
                    src: Local::new(b_path.clone()),
                    dst: Local::new(tmppath!(dst, "b.txt")),
                },
            ];

            let model: CopyModel<'_, Local, Local> = CopyModel::new(actions, || {});
            model.run().expect("Unable to execute model");

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
        }

        #[test]
        fn test_create_file_symbolic() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CopyLink {
                    src: Local::new(a_path.clone()),
                    dst: Local::new(tmppath!(dst, "a.txt")),
                },
                CopyAction::CopyLink {
                    src: Local::new(b_path.clone()),
                    dst: Local::new(tmppath!(dst, "b.txt")),
                },
            ];

            let model: CopyModel<'_, Local, Local> = CopyModel::new(actions, || {});
            model.run().expect("Unable to execute model");

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
            assert!(symlink!(tmppath!(dst, "a.txt")));
            assert!(symlink!(tmppath!(dst, "a.txt")));
        }

        #[test]
        fn test_mixed_model() {
            let src = tmpdir!();
            let dst = tmppath!(src, "target");
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CreateDir {
                    target: Local::new(dst.clone()),
                },
                CopyAction::CopyFile {
                    src: Local::new(a_path),
                    dst: Local::new(dst.join("a.txt")),
                },
                CopyAction::CopyLink {
                    src: Local::new(b_path),
                    dst: Local::new(dst.join("b.txt")),
                },
            ];

            let model: CopyModel<'_, Local, Local> = CopyModel::new(actions, || {});
            model.run().expect("Unable to execute model");

            assert!(tmppath!(src, "target").exists());
            assert!(tmppath!(src, "target").is_dir());
            assert!(tmppath!(src, "target/a.txt").exists());
            assert_eq!(read_file!(tmppath!(src, "target/a.txt")), "aaaa");
            assert!(tmppath!(src, "target/b.txt").exists());
            assert!(symlink!(tmppath!(src, "target/b.txt")));
        }
    }
}
