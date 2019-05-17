use std::fs::{self, OpenOptions};
use std::io::{self, Error, ErrorKind};
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::path::{Display, Path};

use super::{FileSystem, Local, Route};
use crate::config::archive::{Folder, Snapshot};

/// Represents the virtual filesystem used for storing the backups. The main
#[derive(Debug, Clone)]
pub struct Archive {
    path: Local,
}

impl Archive {
    #[allow(missing_docs)]
    pub fn new<P: AsRef<Path>>(root: P, folder: Folder, snapshot: Snapshot) -> Self {
        Self {
            path: Local::new(
                folder
                    .resolve(root)
                    .relative
                    .join(rfc3339!(snapshot.timestamp())),
            ),
        }
    }

    fn from_join_archive<P: AsRef<Path>>(archive: &Archive, path: P) -> Self {
        Self {
            path: archive.path.join(path),
        }
    }
}

impl FileSystem for Archive {
    type File = fs::File;
    type Metadata = fs::Metadata;
    type Directory = fs::DirEntry;
    type DirectoryIterator = fs::ReadDir;

    fn exists(&self) -> bool {
        self.path.exists()
    }

    fn metadata(&self) -> io::Result<Self::Metadata> {
        self.path.metadata()
    }

    fn symlink_metadata(&self) -> io::Result<Self::Metadata> {
        self.path.symlink_metadata()
    }

    fn is_file(&self) -> bool {
        self.path.is_file()
    }

    fn open(&self, options: &OpenOptions) -> io::Result<Self::File> {
        options.open(self.path())
    }

    fn read_dir(&self) -> io::Result<Self::DirectoryIterator> {
        self.path.read_dir()
    }

    fn create_dir_all(&self) -> io::Result<()> {
        self.path.create_dir_all()
    }

    fn remove_file(&self) -> io::Result<()> {
        self.path.remove_file()
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

        symlink(self.path(), other.path())
    }
}

impl Route for Archive {
    fn path(&self) -> &Path {
        self.path.path()
    }

    fn join<T: AsRef<Path>>(&self, other: T) -> Self {
        Archive::from_join_archive(self, other)
    }

    fn display(&self) -> Display<'_> {
        self.path.display()
    }
}

impl PartialEq for Archive {
    fn eq(&self, _other: &Archive) -> bool {
        true
    }
}

impl PartialEq<Local> for Archive {
    fn eq(&self, _other: &Local) -> bool {
        true
    }
}
