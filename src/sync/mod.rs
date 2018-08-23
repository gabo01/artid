use failure::ResultExt;
use logger::pathlight;
use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {AppError, FsError, Result};

mod helpers;
use self::helpers::{sync, Branchable, Linkable};

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
    root: RefCell<DirRoot>,
}

impl DirTree {
    /// Creates a new link representation for two different trees.
    pub fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self {
            root: RefCell::new(DirRoot::new(src, dst)),
        }
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the dst dir, the 'link' or if it is unable to
    /// read the contents of the src, the 'linked', dir.
    ///
    /// Behaviour of these function can be controlled through the options sent for things such
    /// as file clashes, errors while processing a file or a subdirectory and other things. See
    /// SyncOptions docs for more info on these topic.
    pub fn sync(&self, options: SyncOptions) -> Result<()> {
        sync(self, options)
    }
}

impl<'a, 'b> Branchable<'a, DirBranch<'a>, &'b OsString> for DirTree {
    fn branch(&'a self, branch: &'b OsString) -> DirBranch<'a> {
        self.root.borrow_mut().branch(branch);
        DirBranch::new(self)
    }

    fn root(&self) {
        self.root.borrow_mut().root();
    }
}

impl<'a> Linkable<'a, DirRoot> for DirTree {
    type Link = LinkedPoint<'a>;

    fn to_ref(&self) -> Ref<DirRoot> {
        self.root.borrow()
    }

    fn link(&'a self) -> Self::Link {
        LinkedPoint::new(self.root.borrow())
    }
}

/// Represents both roots of the directory trees designed to be linked. In order to make
/// branches be able to hold a mutable instance of this object. This object is put inside
/// a RefCell and handled from there. See DirTree related code for more specific details.
#[derive(Debug)]
struct DirRoot {
    src: PathBuf,
    dst: PathBuf,
}

impl DirRoot {
    /// Creates a new DirRoot given the roots of both trees.
    pub(self) fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self { src, dst }
    }

    /// Switches to a branch of the trees. This operation is to be called only when
    /// generating a new DirBranch object.
    pub(self) fn branch<P: AsRef<Path>>(&mut self, branch: P) {
        let branch = branch.as_ref();
        self.src.push(branch);
        self.dst.push(branch);
    }

    /// Returns to the root of the current branch. Calling this function while being already
    /// at the root of the trees will cause it to malfunction. This should be called only
    /// by a drop implementation of the branch object.
    pub(self) fn root(&mut self) {
        self.src.pop();
        self.dst.pop();
    }
}

/// Represents a branch of the directory tree being iterated over. It is fundamentally a
/// reference to the DirTree that works as a stack during iteration. In order to get
/// interior mutability uses the RefCell inside the DirTree.
#[derive(Debug)]
struct DirBranch<'a> {
    tree: &'a DirTree,
}

impl<'a> DirBranch<'a> {
    /// Creates a new branch from a tree reference. This should be called only from the
    /// branch method of DirTree or another DirBranch in order to do the other needed
    /// operations to create a branch.
    pub(self) fn new(tree: &'a DirTree) -> Self {
        Self { tree }
    }
}

impl<'a, 'b> Branchable<'a, DirBranch<'a>, &'b OsString> for DirBranch<'a> {
    fn branch(&'a self, branch: &'b OsString) -> DirBranch<'a> {
        self.tree.branch(branch)
    }

    fn root(&self) {
        self.tree.root()
    }
}

impl<'a> Drop for DirBranch<'a> {
    fn drop(&mut self) {
        self.root();
    }
}

impl<'a, 'b> Linkable<'b, DirRoot> for DirBranch<'a> {
    type Link = LinkedPoint<'b>;

    fn to_ref(&self) -> Ref<DirRoot> {
        self.tree.to_ref()
    }

    fn link(&'b self) -> Self::Link {
        self.tree.link()
    }
}

/// Represents a link between two different paths points. The dst path is seen as the
/// 'link's location while the src path is seen as the link's pointed place.
#[derive(Debug)]
struct LinkedPoint<'a> {
    pointer: Ref<'a, DirRoot>,
}

impl<'a> LinkedPoint<'a> {
    /// Creates a link representation of two different locations.
    pub(self) fn new(pointer: Ref<'a, DirRoot>) -> Self {
        Self { pointer }
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest.
    pub(self) fn synced(&self) -> bool {
        if self.pointer.src.exists() && self.pointer.dst.exists() {
            if let Some(linked) = modified(&self.pointer.src) {
                if let Some(link) = modified(&self.pointer.dst) {
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
        if overwrite == OverwriteMode::Disallow && self.pointer.dst.exists() {
            err!(FsError::PathExists((&self.pointer.dst).into()));
        }

        if overwrite == OverwriteMode::Allow && self.synced() {
            return Ok(());
        }

        if !symbolic {
            fs::copy(&self.pointer.src, &self.pointer.dst)
                .context(FsError::CreateFile((&self.pointer.dst).into()))?;
        } else {
            Self::symlink(&self.pointer.src, &self.pointer.dst)
                .context(FsError::CreateFile((&self.pointer.dst).into()))?;
        }

        info!(
            "synced: {} -> {}",
            pathlight(&self.pointer.src),
            pathlight(&self.pointer.dst)
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
    extern crate tempfile;

    use super::{modified, Branchable, DirTree, Linkable, LinkedPoint, OverwriteMode};
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::{thread, time};

    #[test]
    fn test_branching() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        let tree = DirTree::new(src.path().into(), dst.path().into());
        {
            let string = OsString::from("codes");
            let _branch = tree.branch(&string);

            assert_eq!(
                tree.root.borrow().src.display().to_string(),
                src.path().join(&string).display().to_string()
            );

            assert_eq!(
                tree.root.borrow().dst.display().to_string(),
                dst.path().join(&string).display().to_string()
            );
        }

        assert_eq!(
            tree.root.borrow().src.display().to_string(),
            src.path().display().to_string()
        );

        assert_eq!(
            tree.root.borrow().dst.display().to_string(),
            dst.path().display().to_string()
        );
    }

    #[test]
    fn test_linked() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let _srcfile = File::create(&srcpath).unwrap();
        let _dstfile = File::create(&dstpath).unwrap();
        let tree = DirTree::new(srcpath, dstpath);
        let link = LinkedPoint::new(tree.root.borrow());
        assert!(link.synced());
    }

    #[test]
    fn test_link_generation_linked() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let _srcfile = File::create(&srcpath).unwrap();
        let _dstfile = File::create(&dstpath).unwrap();
        let tree = DirTree::new(srcpath, dstpath);
        assert!(tree.link().synced());
    }

    #[test]
    fn test_mirror_disallowed() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let _srcfile = File::create(&srcpath).unwrap();
        let _dstfile = File::create(&dstpath).unwrap();
        let tree = DirTree::new(srcpath, dstpath);
        assert!(tree.link().mirror(OverwriteMode::Disallow).is_err());
    }

    #[test]
    fn test_mirror_allow_copy() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let dstfile = File::create(&dstpath).unwrap();
        ::std::mem::drop(dstfile);
        thread::sleep(time::Duration::from_millis(2000));

        let mut srcfile = File::create(&srcpath).unwrap();
        write!(srcfile, "Hello, world").unwrap();
        ::std::mem::drop(srcfile);

        assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

        let tree = DirTree::new(srcpath.clone(), dstpath.clone());
        assert!(
            tree.link().mirror(OverwriteMode::Allow).is_ok(),
            "Mirror was not successful"
        );

        let mut dstfile = File::open(&dstpath).unwrap();
        let mut string = String::new();
        dstfile.read_to_string(&mut string).unwrap();
        assert_eq!(string, String::from("Hello, world"));
    }

    #[test]
    fn test_mirror_allow_not_copy() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let mut srcfile = File::create(&srcpath).unwrap();
        write!(srcfile, "Hello, world").unwrap();
        ::std::mem::drop(srcfile);
        thread::sleep(time::Duration::from_millis(2000));

        let dstfile = File::create(&dstpath).unwrap();
        ::std::mem::drop(dstfile);

        assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

        let tree = DirTree::new(srcpath.clone(), dstpath.clone());
        assert!(
            tree.link().mirror(OverwriteMode::Allow).is_ok(),
            "Mirror was not successful"
        );

        let mut dstfile = File::open(&dstpath).unwrap();
        let mut string = String::new();
        dstfile.read_to_string(&mut string).unwrap();
        assert_ne!(string, String::from("Hello, world"));
    }

    #[test]
    fn test_mirror_force() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let mut srcfile = File::create(&srcpath).unwrap();
        write!(srcfile, "Hello, world").unwrap();
        ::std::mem::drop(srcfile);
        thread::sleep(time::Duration::from_millis(2000));

        let dstfile = File::create(&dstpath).unwrap();
        ::std::mem::drop(dstfile);

        assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

        let tree = DirTree::new(srcpath.clone(), dstpath.clone());
        assert!(
            tree.link().mirror(OverwriteMode::Force).is_ok(),
            "Mirror was not successful"
        );

        let mut dstfile = File::open(&dstpath).unwrap();
        let mut string = String::new();
        dstfile.read_to_string(&mut string).unwrap();
        assert_eq!(string, String::from("Hello, world"));
    }

    #[test]
    fn test_mirror_force_allow() {
        let dir = tempfile::tempdir().unwrap();
        let srcpath = dir.path().join("a.txt");
        let dstpath = dir.path().join("b.txt");

        let dstfile = File::create(&dstpath).unwrap();
        ::std::mem::drop(dstfile);
        thread::sleep(time::Duration::from_millis(2000));

        let mut srcfile = File::create(&srcpath).unwrap();
        write!(srcfile, "Hello, world").unwrap();
        ::std::mem::drop(srcfile);

        assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

        let tree = DirTree::new(srcpath.clone(), dstpath.clone());
        assert!(
            tree.link().mirror(OverwriteMode::Force).is_ok(),
            "Mirror was not successful"
        );

        let mut dstfile = File::open(&dstpath).unwrap();
        let mut string = String::new();
        dstfile.read_to_string(&mut string).unwrap();
        assert_eq!(string, String::from("Hello, world"));
    }
}
