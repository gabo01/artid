use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Display, Path, PathBuf};

mod local;

pub use self::local::Local;

/// Abstraction over a filesystem. Allows to use different representations of a file
/// system for artid's operations.
/// 
/// All the functions present in this trait are present in the standard library and are
/// intended to behave in a similar way to the ones implemented there
pub trait FileSystem: Route {
    #[allow(missing_docs)]
    fn new<P: Into<PathBuf>>(path: P) -> Self;

    #[allow(missing_docs)]
    fn exists(&self) -> bool;

    #[allow(missing_docs)]
    fn metadata(&self) -> io::Result<fs::Metadata>;

    #[allow(missing_docs)]
    fn symlink_metadata(&self) -> io::Result<fs::Metadata>;

    #[allow(missing_docs)]
    fn is_file(&self) -> bool;

    #[allow(missing_docs)]
    fn open(&self, options: &OpenOptions) -> io::Result<fs::File>;

    #[allow(missing_docs)]
    fn read_dir(&self) -> io::Result<fs::ReadDir>;

    #[allow(missing_docs)]
    fn create_dir_all(&self) -> io::Result<()>;

    #[allow(missing_docs)]
    fn remove_file(&self) -> io::Result<()>;

    #[allow(missing_docs)]
    fn symlink_to<F>(&self, other: &F) -> io::Result<()>
    where
        Self: PartialEq<F>,
        F: FileSystem;

    #[allow(missing_docs)]
    fn copy_to<F: FileSystem>(&self, other: &F) -> io::Result<u64> {
        let mut reader = self.open(fs::OpenOptions::new().read(true))?;
        let mut writer = other.open(
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true),
        )?;

        let perm = reader.metadata()?.permissions();
        let bytes = io::copy(&mut reader, &mut writer)?;
        writer.set_permissions(perm)?;

        Ok(bytes)
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
