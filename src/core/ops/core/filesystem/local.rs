use std::ffi::OsString;
use std::fmt::Debug;
use std::fs::{self, OpenOptions};
use std::io::{self, Error, ErrorKind};
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::path::{Display, Path, PathBuf};
use std::time::SystemTime;

use super::{Directory, DirectoryIterator, File, FileKind, FileSystem, Metadata, Route};

/// Represents the standard filesystem. As such, the methods implemented here are
/// fundamentally call's to the functions in the standard library
#[derive(Debug, Clone)]
pub struct Local {
    path: PathBuf,
}

impl FileSystem for Local {
    type File = fs::File;
    type Metadata = fs::Metadata;
    type Directory = fs::DirEntry;
    type DirectoryIterator = fs::ReadDir;

    fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }

    fn exists(&self) -> bool {
        self.path.exists()
    }

    fn metadata(&self) -> io::Result<Self::Metadata> {
        self.path.metadata()
    }

    fn symlink_metadata(&self) -> io::Result<Self::Metadata> {
        fs::symlink_metadata(&self.path)
    }

    fn is_file(&self) -> bool {
        self.path.is_file()
    }

    fn open(&self, options: &OpenOptions) -> io::Result<Self::File> {
        options.open(&self.path)
    }

    fn read_dir(&self) -> io::Result<Self::DirectoryIterator> {
        fs::read_dir(&self.path)
    }

    fn create_dir_all(&self) -> io::Result<()> {
        fs::create_dir_all(&self.path)
    }

    fn remove_file(&self) -> io::Result<()> {
        fs::remove_file(&self.path)
    }

    fn symlink_to<F>(&self, other: &F) -> io::Result<()>
    where
        Self: PartialEq<F>,
        F: FileSystem,
    {
        const MESSAGE: &str = "A symlink can't be made between two different filesystems";

        if !(self == other) {
            return Err(Error::new(ErrorKind::InvalidInput, MESSAGE));
        }

        symlink(&self.path, other.path())
    }
}

impl Route for Local {
    fn path(&self) -> &Path {
        &self.path
    }

    fn join<T: AsRef<Path>>(&self, other: T) -> Self {
        Local::new(self.path.join(other))
    }

    fn display(&self) -> Display<'_> {
        self.path.display()
    }
}

impl PartialEq for Local {
    fn eq(&self, _other: &Local) -> bool {
        true
    }
}

impl File for fs::File {
    type Metadata = fs::Metadata;

    fn metadata(&self) -> io::Result<Self::Metadata> {
        self.metadata()
    }
}

impl Metadata for fs::Metadata {
    type FileKind = fs::FileType;

    fn file_type(&self) -> Self::FileKind {
        self.file_type()
    }

    fn modified(&self) -> io::Result<SystemTime> {
        self.modified()
    }
}

impl FileKind for fs::FileType {
    fn is_file(&self) -> bool {
        self.is_file()
    }

    fn is_symlink(&self) -> bool {
        self.is_symlink()
    }

    fn is_dir(&self) -> bool {
        self.is_dir()
    }
}

impl DirectoryIterator<fs::DirEntry> for fs::ReadDir {}

impl Directory for fs::DirEntry {
    fn path(&self) -> PathBuf {
        self.path()
    }

    fn file_name(&self) -> OsString {
        self.file_name()
    }
}
