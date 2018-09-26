use failure::{Fail, ResultExt};
use logger::pathlight;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {AppError, FsError, Result};

/// Used to handle errors in the sync process.
macro_rules! handle_errors {
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

/// Modifier options for the sync process in a DirTree. Check the properties to see which
/// behaviour they control.
#[derive(Debug, Copy, Clone)]
pub struct SyncOptions {
    /// Enables/Disables warnings on the sync process. If an error is raises while processing
    /// the sync: a folder can't be read from (excluding the main folders), the user does not
    /// have permissions for accessing a file, the function will emit a warning instead of
    /// exiting with an error.
    ///
    /// In short words: (warn == true) => function will warn about errors instead of failing the
    /// backup operation.
    pub warn: bool,
    /// Enables/Disables cleanup of files. If a file is present on the location to be written
    /// on but does not exist in it's supposed original location, the file will be deleted from
    /// the backup. This avoids generating garbage files on a backup dir.
    pub clean: bool,
    /// Controls how to handle if a location to be written on already exists. See OverwriteMode
    /// docs for more info on how this setting behaves.
    pub overwrite: OverwriteMode,
    /// Enables/Disables sync through symbolic links. If set to true a symbolic link will be
    /// created in the destination instead of copying the whole file.
    pub symbolic: bool,
}

impl SyncOptions {
    /// Creates a new set of options for the sync process.
    pub fn new(warn: bool, clean: bool, overwrite: OverwriteMode) -> Self {
        Self {
            warn,
            clean,
            overwrite,
            symbolic: false,
        }
    }
}

/// Sets the mode for handling the case in which a file would be overwritten by the sync
/// operation.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OverwriteMode {
    /// The function will raise an error if the location where it tries to write already
    /// exists.
    Disallow,
    /// The function will compare both locations last modification date. If the location
    /// to be written on is older than the location whose contents will be copied the
    /// location will be overwritten.
    Allow,
    /// The fuction will always overwrite the destination location regardless of the last
    /// modification date or any other parameter.
    Force,
}

/// Represents two different linked directory trees. The dst path is seen as the 'link'
/// and the src path is seen as the 'linked place'. This means that syncing the link is making
/// a copy of all files in src to dst.
///
/// The idea behind this type is to be able to walk the dest path and mimic it's structure on
/// the origin path.
///
/// Creation of this type won't fail even if the given path's aren't valid. You can check if the
/// given path's are correct by calling .valid(). If the path's are not correct the sync function
/// will fail and return an appropiate error.
#[derive(Debug)]
pub struct DirTree {
    src: PathBuf,
    dst: PathBuf,
}

impl DirTree {
    /// Creates a new link representation for two different trees.
    pub fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self { src, dst }
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the dst dir, the 'link' or if it is unable to
    /// read the contents of the src, the 'linked', dir.
    ///
    /// Behaviour of these function can be controlled through the options sent for things such
    /// as file clashes, errors while processing a file or a subdirectory and other things. See
    /// SyncOptions docs for more info on these topic.
    pub fn sync(self, options: SyncOptions) -> Result<(SyncModel)> {
        let mut actions = vec![];

        for entry in Self::walk(&self.src, options.warn)? {
            match entry.kind() {
                FileSystemType::Dir => {
                    if !self.dst.join(&entry.path).exists() {
                        actions.push(SyncActions::CreateDir(entry.path))
                    }
                }

                FileSystemType::File => actions.push(SyncActions::LinkFile(entry.path)),

                FileSystemType::Other => {
                    warn!("Unable to process {}", pathlight(entry.full_path()))
                }
            }
        }

        if options.clean && self.dst.exists() {
            for entry in Self::walk(&self.dst, options.warn)? {
                match entry.kind() {
                    FileSystemType::Dir | FileSystemType::File => {
                        if !self.src.join(&entry.path).exists() {
                            actions.push(SyncActions::DeleteDst(entry.path))
                        }
                    }

                    FileSystemType::Other => {
                        warn!("Unable to process {}", pathlight(entry.full_path()))
                    }
                }
            }
        }

        Ok(SyncModel::new(self, actions, options))
    }

    fn walk(dir: &Path, warn: bool) -> Result<Vec<Entry<'_>>> {
        let mut entries = vec![Entry::new(dir, "".into(), 0)];
        let mut walked = Self::walk_recursive(&entries[0], warn)?;
        entries.append(&mut walked);
        Ok(entries)
    }

    fn walk_recursive<'a>(entry: &Entry<'a>, warn: bool) -> Result<Vec<Entry<'a>>> {
        let mut entries = vec![];

        for element in fs::read_dir(entry.full_path())
            .context(FsError::ReadFile(entry.full_path().into()))?
            .into_iter()
            .filter_map(|e| e.ok())
        {
            entries.push(Entry::new(
                entry.root,
                entry.path.join(element.file_name()),
                entry.deepness + 1,
            ));

            if let FileSystemType::Dir = FileSystemType::from(element.path()) {
                match Self::walk_recursive(&entries.last().unwrap(), warn) {
                    Ok(mut walked) => entries.append(&mut walked),
                    Err(err) => handle_errors!(
                        warn,
                        err,
                        "Unable to read {}",
                        pathlight(entries.last().unwrap().full_path())
                    ),
                }
            }
        }

        Ok(entries)
    }
}

struct Entry<'a> {
    root: &'a Path,
    path: PathBuf,
    deepness: u8,
}

impl<'a> Entry<'a> {
    pub fn new(root: &'a Path, path: PathBuf, deepness: u8) -> Self {
        Self {
            root,
            path,
            deepness,
        }
    }

    pub fn kind(&self) -> FileSystemType {
        FileSystemType::from(self.root.join(&self.path))
    }

    pub fn full_path(&self) -> PathBuf {
        self.root.join(&self.path)
    }
}

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

pub enum SyncActions {
    CreateDir(PathBuf),
    LinkFile(PathBuf),
    DeleteDst(PathBuf),
}

pub struct SyncModel {
    src: PathBuf,
    dst: PathBuf,
    actions: Vec<SyncActions>,
    overwrite: OverwriteMode,
    symbolic: bool,
}

impl SyncModel {
    pub fn new(tree: DirTree, actions: Vec<SyncActions>, options: SyncOptions) -> Self {
        Self {
            src: tree.src,
            dst: tree.dst,
            actions,
            overwrite: options.overwrite,
            symbolic: options.symbolic,
        }
    }

    pub fn execute(self) -> Result<()> {
        for action in self.actions {
            match action {
                SyncActions::CreateDir(dir) => fs::create_dir_all(self.dst.join(dir))
                    .context(FsError::OpenFile((&self.dst).into()))?,

                SyncActions::LinkFile(ref link) => {
                    LinkedPoint::new(self.src.join(link), self.dst.join(link))
                        .mirror(self.overwrite, self.symbolic)?;
                }

                SyncActions::DeleteDst(ref path) => {
                    let full_path = self.dst.join(path);

                    match FileSystemType::from(&full_path) {
                        FileSystemType::Dir => {
                            fs::remove_dir_all(&full_path)
                                .context(FsError::DeleteFile(full_path.into()))?;
                        }

                        FileSystemType::File => {
                            fs::remove_file(&full_path)
                                .context(FsError::DeleteFile(full_path.into()))?;
                        }

                        FileSystemType::Other => {
                            warn!("Unable to identify {}", pathlight(full_path))
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Represents a link between two different paths points. The dst path is seen as the
/// 'link's location while the src path is seen as the link's pointed place.
#[derive(Debug)]
struct LinkedPoint {
    src: PathBuf,
    dst: PathBuf,
}

impl LinkedPoint {
    /// Creates a link representation of two different locations.
    pub(self) fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self { src, dst }
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest.
    pub(self) fn synced(&self) -> bool {
        if self.src.exists() && self.dst.exists() {
            if let Some(linked) = modified(&self.src) {
                if let Some(link) = modified(&self.dst) {
                    return link >= linked;
                }
            }
        }

        false
    }

    /// Syncs (or Links) the two points on the filesystem. The behaviour of this function
    /// for making the sync is controlled by the overwrite option. See the docs for
    /// OverwriteMode to get more info.
    ///
    /// The behaviour is also controlled by the symbolic parameter. If set to true the
    /// function will create a symbolic link instead of copying the file.
    pub(self) fn mirror(&self, overwrite: OverwriteMode, symbolic: bool) -> Result<()> {
        if overwrite == OverwriteMode::Disallow && self.dst.exists() {
            err!(FsError::PathExists((&self.dst).into()));
        }

        if overwrite == OverwriteMode::Allow && self.synced() {
            return Ok(());
        }

        if !symbolic {
            if let Ok(metadata) = fs::symlink_metadata(&self.dst) {
                if metadata.file_type().is_symlink() {
                    fs::remove_file(&self.dst).context(FsError::DeleteFile((&self.dst).into()))?;
                }
            }

            fs::copy(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        } else {
            Self::symlink(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        }

        info!(
            "synced: {} -> {}",
            pathlight(&self.src),
            pathlight(&self.dst)
        );

        Ok(())
    }

    /// Intended to create a symlink on Unix operating systems
    #[cfg(unix)]
    fn symlink<P: AsRef<Path>, T: AsRef<Path>>(src: P, dst: T) -> ::std::io::Result<()> {
        use std::os::unix::fs::symlink;
        symlink(src, dst)
    }

    /// Intended to create a symlink on Windows operating systems
    #[cfg(windows)]
    fn symlink<P: AsRef<Path>, T: AsRef<Path>>(src: P, dst: T) -> ::std::io::Result<()> {
        use std::os::windows::fs::symlink_file as symlink;
        symlink(src, dst)
    }
}

/// Queries the filesystem and gets the date of the last time the file was modified keeped
/// by the system. Since this is a measurement made by the system, the time returned by this
/// function can be wrong in some cases: the user changed the date in it's system, an operation
/// was queued and performed at a later time and some other cases.
fn modified<P: AsRef<Path>>(file: P) -> Option<SystemTime> {
    if let Ok(data) = file.as_ref().metadata() {
        if let Ok(time) = data.modified() {
            return Some(time);
        }
    }

    warn!("Unable to access metadata for {}", pathlight(file.as_ref()));
    None
}

#[cfg(test)]
mod tests {
    use super::{modified, DirTree, FileSystemType, LinkedPoint, OverwriteMode, SyncOptions};

    mod file_system {
        use super::FileSystemType;
        use std::fs::File;
        use tempfile;

        #[test]
        fn test_system_dir() {
            let dir = tmpdir!();
            assert_eq!(FileSystemType::from(dir.path()), FileSystemType::Dir);
        }

        #[test]
        fn test_system_file() {
            let dir = tmpdir!();
            let path = create_file!(tmppath!(dir, "a.txt"));
            assert_eq!(FileSystemType::from(path), FileSystemType::File);
        }
    }

    mod linked_point {
        use super::{modified, LinkedPoint, OverwriteMode};
        use std::{fs::File, thread, time};
        use tempfile;

        #[test]
        fn test_linked() {
            let dir = tmpdir!();
            let srcpath = create_file!(tmppath!(dir, "a.txt"));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            let link = LinkedPoint::new(srcpath, dstpath);
            assert!(link.synced());
        }

        #[test]
        fn test_mirror_disallowed() {
            let dir = tmpdir!();
            let srcpath = create_file!(tmppath!(dir, "a.txt"));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            let link = LinkedPoint::new(srcpath, dstpath);
            assert!(link.mirror(OverwriteMode::Disallow, false).is_err());
        }

        #[test]
        fn test_mirror_allow_copy() {
            let dir = tmpdir!();

            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            thread::sleep(time::Duration::from_millis(2000));
            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Allow, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_allow_not_copy() {
            let dir = tmpdir!();

            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            thread::sleep(time::Duration::from_millis(2000));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Allow, false).is_ok());

            assert_ne!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_force() {
            let dir = tmpdir!();

            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            thread::sleep(time::Duration::from_millis(2000));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Force, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_force_allow() {
            let dir = tmpdir!();

            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            thread::sleep(time::Duration::from_millis(2000));
            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Force, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }
    }

    mod sync_op {
        use super::{DirTree, OverwriteMode, SyncOptions};
        use std::fs::{self, File};
        use tempfile;

        #[test]
        fn test_sync_copy_single_dir() {
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");

            DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap()
                .execute()
                .unwrap();

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
        }

        #[test]
        fn test_sync_copy_symbolic() {
            let mut options = SyncOptions::new(false, false, OverwriteMode::Force);
            options.symbolic = true;

            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");

            DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap()
                .execute()
                .unwrap();

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert!(symlink!(tmppath!(dst, "a.txt")));
            assert!(symlink!(tmppath!(dst, "b.txt")));
        }

        #[test]
        fn test_sync_copy_recursive() {
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            let (src, dst) = (tmpdir!(), tmpdir!());
            fs::create_dir(tmppath!(src, "c")).expect("Unable to create folder");
            create_file!(tmppath!(src, "c/d.txt"), "dddd");

            DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap()
                .execute()
                .unwrap();

            assert!(tmppath!(dst, "c/d.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "c/d.txt")), "dddd");
        }

        #[test]
        fn test_sync_copy_symbolic_recursive() {
            let mut options = SyncOptions::new(false, false, OverwriteMode::Force);
            options.symbolic = true;

            let (src, dst) = (tmpdir!(), tmpdir!());
            fs::create_dir(tmppath!(src, "c")).expect("Unable to create folder");
            create_file!(tmppath!(src, "c/d.txt"), "dddd");

            DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap()
                .execute()
                .unwrap();

            assert!(dst.path().join("c/d.txt").exists());
            assert!(symlink!(tmppath!(dst, "c/d.txt")));
        }
    }
}
