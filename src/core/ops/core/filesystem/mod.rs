//! Implementation of the abstractions over the filesystem.
//!
//! These abstractions are designed to allow for the implementation of multiple
//! filesystem representations that share behavioral traits such as a zip file
//! or a remote server.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Display, Path, PathBuf};
use std::time::SystemTime;

mod local;

pub use self::local::Local;

/// Abstraction over a filesystem. Allows to use different representations of a file
/// system for artid's operations.
///
/// All the functions present in this trait are present in the standard library and are
/// intended to behave in a similar way to the ones implemented there
pub trait FileSystem: Route {
    #[allow(missing_docs)]
    type File: File<Metadata = Self::Metadata>;

    #[allow(missing_docs)]
    type Metadata: Metadata;

    #[allow(missing_docs)]
    type Directory: Directory;

    #[allow(missing_docs)]
    type DirectoryIterator: DirectoryIterator<Self::Directory>;

    #[allow(missing_docs)]
    fn new<P: Into<PathBuf>>(path: P) -> Self;

    #[allow(missing_docs)]
    fn exists(&self) -> bool;

    #[allow(missing_docs)]
    fn metadata(&self) -> io::Result<Self::Metadata>;

    #[allow(missing_docs)]
    fn symlink_metadata(&self) -> io::Result<Self::Metadata>;

    #[allow(missing_docs)]
    fn is_file(&self) -> bool;

    #[allow(missing_docs)]
    fn open(&self, options: &OpenOptions) -> io::Result<Self::File>;

    #[allow(missing_docs)]
    fn read_dir(&self) -> io::Result<Self::DirectoryIterator>;

    #[allow(missing_docs)]
    fn create_dir_all(&self) -> io::Result<()>;

    #[allow(missing_docs)]
    fn remove_file(&self) -> io::Result<()>;

    #[allow(missing_docs)]
    fn symlink_to<F>(&self, other: &F) -> io::Result<()>
    where
        Self: PartialEq<F>,
        F: FileSystem;

    /// Mimics the behaviour of the copy function for two filesystems with the only
    /// drawback that it does not copy the permissions from the source to the
    /// destination
    fn copy_to<F: FileSystem>(&self, other: &F) -> io::Result<u64> {
        let mut reader = self.open(fs::OpenOptions::new().read(true))?;
        let mut writer = other.open(
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true),
        )?;

        io::copy(&mut reader, &mut writer)
    }
}

/// Abstraction over a path. It implements an abstraction over the path methods that
/// do not need to touch the filesystem. If the required method has to do a system call,
/// then it goes inside the FileSystem trait and not this one
pub trait Route {
    #[allow(missing_docs)]
    fn path(&self) -> &Path;

    #[allow(missing_docs)]
    fn join<P: AsRef<Path>>(&self, other: P) -> Self;

    #[allow(missing_docs)]
    fn display(&self) -> Display<'_>;
}

/// Representation of the open file objects inside a filesystem
pub trait File: io::Read + io::Write {
    #[allow(missing_docs)]
    type Metadata: Metadata;

    #[allow(missing_docs)]
    fn metadata(&self) -> io::Result<Self::Metadata>;
}

/// Representation of the metadata inside a path for a particular filesystem
pub trait Metadata {
    #[allow(missing_docs)]
    type FileKind: FileKind;

    #[allow(missing_docs)]
    fn file_type(&self) -> Self::FileKind;

    #[allow(missing_docs)]
    fn modified(&self) -> io::Result<SystemTime>;
}

/// Representation of the filekind object for a particular filesystem
pub trait FileKind {
    #[allow(missing_docs)]
    fn is_symlink(&self) -> bool;

    #[allow(missing_docs)]
    fn is_file(&self) -> bool;

    #[allow(missing_docs)]
    fn is_dir(&self) -> bool;
}

#[allow(missing_docs)]
pub trait DirectoryIterator<D: Directory>: Iterator<Item = io::Result<D>> {}

/// Representation of a directory in the filesystem
pub trait Directory {
    #[allow(missing_docs)]
    fn path(&self) -> PathBuf;

    #[allow(missing_docs)]
    fn file_name(&self) -> OsString;
}
