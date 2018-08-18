#![allow(dead_code)]

use failure::{Fail, ResultExt};
use logger::pathlight;
use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {AppError, AppErrorType, Result};

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
trait Branchable<'a, T: 'a, P> {
    fn branch(&'a self, branch: P) -> T;
    fn root(&self);
}

/// Used to give an object the ability to represent a link between two locations.
trait Linkable<T> {
    type Link;

    fn valid(&self) -> bool;
    fn to_ref(&self) -> Ref<T>;
    fn link(&self) -> Self::Link;
}

/// Internal recursive function used to sync two trees by using branches. See the docs of
/// DirTree::sync to understand how this function works on a general level.
fn sync<'a, T, O>(tree: &'a T, options: O) -> Result<()>
where
    T: 'a
        + for<'b> Branchable<'a, DirBranch<'a>, &'b OsString>
        + Linkable<DirRoot, Link = LinkedPoint>,
    O: Into<SyncOptions>,
{
    let mut options = options.into();

    debug!(
        "Syncing {} with {}",
        pathlight(&tree.to_ref().dest),
        pathlight(&tree.to_ref().origin)
    );

    if !tree.valid() {
        fs::create_dir_all(&tree.to_ref().origin).context("Unable to create backup dir")?;
        options.clean = false; // no need to perform the clean check if the dir is empty
    }

    let dest = tree.to_ref().dest.clone();
    for entry in fs::read_dir(dest).context("Unable to read dir")? {
        match entry {
            Ok(component) => {
                let branch = tree.branch(&component.file_name());
                match FileSystemType::from(&branch.to_ref().dest) {
                    FileSystemType::File => {
                        if let Err(err) = branch.link().mirror(options.overwrite) {
                            handle!(
                                options.warn,
                                err,
                                "Unable to copy {}",
                                pathlight(&branch.to_ref().dest)
                            );
                        }
                    }

                    FileSystemType::Dir => {
                        if let Err(err) = sync(&branch, options) {
                            handle!(
                                options.warn,
                                err,
                                "Unable to read {}",
                                pathlight(&branch.to_ref().dest)
                            );
                        }
                    }

                    FileSystemType::Other => {
                        warn!("Unable to process {}", pathlight(&branch.to_ref().dest));
                    }
                };
            }

            Err(_) => warn!("Unable to read entry"),
        }
    }

    if options.clean {
        clean(tree);
    }

    Ok(())
}

/// Internal recursive function used to clean the backup directory of garbage files.
fn clean<'a, T>(tree: &'a T)
where
    T: 'a
        + for<'b> Branchable<'a, DirBranch<'a>, &'b OsString>
        + Linkable<DirRoot, Link = LinkedPoint>,
{
    let origin = tree.to_ref().origin.clone();
    if let Ok(val) = fs::read_dir(origin) {
        for entry in val {
            match entry {
                Ok(component) => {
                    let branch = tree.branch(&component.file_name());

                    if !branch.to_ref().dest.exists() {
                        debug!(
                            "Unnexistant {}, removing {}",
                            pathlight(&branch.to_ref().dest),
                            pathlight(&branch.to_ref().origin)
                        );

                        if branch.to_ref().origin.is_dir() {
                            if let Err(err) = fs::remove_dir_all(&branch.to_ref().origin) {
                                error!("{}", err);
                                warn!(
                                    "Unable to remove garbage location {}",
                                    pathlight(&branch.to_ref().origin)
                                );
                            }
                        } else {
                            if let Err(err) = fs::remove_file(&branch.to_ref().origin) {
                                error!("{}", err);
                                warn!(
                                    "Unable to remove garbage location {}",
                                    pathlight(&branch.to_ref().origin)
                                );
                            }
                        }
                    }

                    if let FileSystemType::Dir = FileSystemType::from(&branch.to_ref().origin) {
                        clean(&branch);
                    };
                }

                // FIXME: improve the handle of this case
                Err(_) => warn!("Unable to read entry 2"),
            }
        }
    }
}

/// Modifier options for the sync process in a LinkTree. Check the properties to see which
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
}

impl SyncOptions {
    /// Creates a new set of options for the sync process.
    pub fn new(warn: bool, clean: bool, overwrite: OverwriteMode) -> Self {
        Self {
            warn,
            clean,
            overwrite,
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

/// Represents two different linked directory trees. The origin path is seen as the 'link'
/// and the dest path is seen as the 'linked place'. This means that syncing the link is making
/// a copy of all files in dest to origin.
///
/// The idea behind this type is to be able to walk the dest path and mimic it's structure on
/// the origin path.
///
/// Creation of this type won't fail even if the given path's aren't valid. You can check if the
/// given path's are correct by calling .linked(). If the path's are not correct the sync function
/// will fail and return an appropiate error.
#[derive(Debug)]
pub struct LinkTree {
    origin: PathBuf,
    dest: PathBuf,
}

impl LinkTree {
    /// Creates a new link representation for two different trees.
    pub fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self { origin, dest }
    }

    /// Checks if the tree is valid. The tree is valid if the two points are directories.
    pub fn valid(&self) -> bool {
        self.origin.is_dir() && self.dest.is_dir()
    }

    /// Creates an internal representation of the branch of the tree.
    fn branch<P: AsRef<Path>>(&mut self, branch: &P) {
        self.origin.push(&branch);
        self.dest.push(&branch);
    }

    /// Returns to the root of the currently branch.
    fn root(&mut self) {
        self.origin.pop();
        self.dest.pop();
    }

    /// Creates a link object between the current points in the tree.
    fn link(&self) -> LinkedPoint {
        LinkedPoint::new(&self.origin, &self.dest)
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the destination dir, the 'link' or if it is unable to
    /// read the contents of the origin, the 'linked', dir.
    ///
    /// Behaviour of these function can be controlled through the options sent for things such
    /// as file clashes, errors while processing a file or a subdirectory and other things. See
    /// SyncOptions docs for more info on these topic.
    pub fn sync<T: Into<SyncOptions>>(&mut self, options: T) -> Result<()> {
        let mut options = options.into();

        debug!(
            "Syncing {} with {}",
            pathlight(&self.dest),
            pathlight(&self.origin)
        );

        if !self.valid() {
            fs::create_dir_all(&self.origin).context("Unable to create backup dir")?;
            options.clean = false; // no need to perform the clean check if the dir is empty
        }

        for entry in fs::read_dir(&self.dest).context("Unable to read dir")? {
            match entry {
                Ok(component) => {
                    self.branch(&component.file_name());

                    match FileSystemType::from(&self.dest) {
                        FileSystemType::File => {
                            if let Err(err) = self.link().mirror(options.overwrite) {
                                handle!(
                                    options.warn,
                                    err,
                                    "Unable to copy {}",
                                    pathlight(&self.dest)
                                );
                            }
                        }

                        FileSystemType::Dir => {
                            if let Err(err) = self.sync(options) {
                                handle!(
                                    options.warn,
                                    err,
                                    "Unable to read {}",
                                    pathlight(&self.dest)
                                );
                            }
                        }

                        FileSystemType::Other => {
                            warn!("Unable to process {}", pathlight(&self.dest));
                        }
                    }

                    self.root();
                }

                Err(_) => warn!("Unable to read entry"),
            }
        }

        if options.clean {
            self.clean_backup();
        }

        Ok(())
    }

    /// Cleans garbage files in a backup directory. A file is seen as garbage if it was
    /// removed from the original location.
    fn clean_backup(&mut self) {
        if let Ok(val) = fs::read_dir(&self.origin) {
            for entry in val {
                match entry {
                    Ok(component) => {
                        self.branch(&component.file_name());

                        if !self.dest.exists() {
                            debug!(
                                "Unnexistant {}, removing {}",
                                pathlight(&self.dest),
                                pathlight(&self.origin)
                            );

                            if self.origin.is_dir() {
                                if let Err(err) = fs::remove_dir_all(&self.origin) {
                                    error!("{}", err);
                                    warn!(
                                        "Unable to remove garbage location {}",
                                        pathlight(&self.origin)
                                    );
                                }
                            } else {
                                if let Err(err) = fs::remove_file(&self.origin) {
                                    error!("{}", err);
                                    warn!(
                                        "Unable to remove garbage location {}",
                                        pathlight(&self.origin)
                                    );
                                }
                            }
                        }

                        if let FileSystemType::Dir = FileSystemType::from(&self.origin) {
                            self.clean_backup();
                        }

                        self.root();
                    }

                    // FIXME: improve the handle of this case
                    Err(_) => warn!("Unable to read entry 2"),
                }
            }
        }
    }
}

/// Represents two different linked directory trees. The origin path is seen as the 'link'
/// and the dest path is seen as the 'linked place'. This means that syncing the link is making
/// a copy of all files in dest to origin.
///
/// The idea behind this type is to be able to walk the dest path and mimic it's structure on
/// the origin path.
///
/// Creation of this type won't fail even if the given path's aren't valid. You can check if the
/// given path's are correct by calling .valid(). If the path's are not correct the sync function
/// will fail and return an appropiate error.
pub struct DirTree {
    root: RefCell<DirRoot>,
}

impl DirTree {
    /// Creates a new link representation for two different trees.
    pub fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self {
            root: RefCell::new(DirRoot::new(origin, dest)),
        }
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the destination dir, the 'link' or if it is unable to
    /// read the contents of the origin, the 'linked', dir.
    ///
    /// Behaviour of these function can be controlled through the options sent for things such
    /// as file clashes, errors while processing a file or a subdirectory and other things. See
    /// SyncOptions docs for more info on these topic.
    pub fn sync<T: Into<SyncOptions>>(&self, options: T) -> Result<()> {
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

impl Linkable<DirRoot> for DirTree {
    type Link = LinkedPoint;

    fn valid(&self) -> bool {
        self.root.borrow().valid()
    }

    fn to_ref(&self) -> Ref<DirRoot> {
        self.root.borrow()
    }

    fn link(&self) -> Self::Link {
        let root = self.root.borrow();
        LinkedPoint::new(&root.origin, &root.dest)
    }
}

/// Represents both roots of the directory trees designed to be linked. In order to make
/// branches be able to hold a mutable instance of this object. This object is put inside
/// a RefCell and handled from there. See DirTree related code for more specific details.
struct DirRoot {
    origin: PathBuf,
    dest: PathBuf,
}

impl DirRoot {
    /// Creates a new DirRoot given the roots of both trees.
    pub(self) fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self { origin, dest }
    }

    /// Switches to a branch of the trees. This operation is to be called only when
    /// generating a new DirBranch object.
    pub(self) fn branch<P: AsRef<Path>>(&mut self, branch: P) {
        let branch = branch.as_ref();
        self.origin.push(branch);
        self.dest.push(branch);
    }

    /// Returns to the root of the current branch. Calling this function while being already
    /// at the root of the trees will cause it to malfunction. This should be called only
    /// by a drop implementation of the branch object.
    pub(self) fn root(&mut self) {
        self.origin.pop();
        self.dest.pop();
    }

    /// Confirms the validity of the link. A DirRoot link is valid only if both points are
    /// directories.
    pub(self) fn valid(&self) -> bool {
        self.origin.is_dir() && self.dest.is_dir()
    }
}

/// Represents a branch of the directory tree being iterated over. It is fundamentally a
/// reference to the DirTree that works as an stack during iteration. In order to access
/// mutability uses the RefCell inside the DirTree.
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

impl<'a> Linkable<DirRoot> for DirBranch<'a> {
    type Link = LinkedPoint;

    fn valid(&self) -> bool {
        self.tree.valid()
    }

    fn to_ref(&self) -> Ref<DirRoot> {
        self.tree.to_ref()
    }

    fn link(&self) -> Self::Link {
        self.tree.link()
    }
}

/// Represents a link between two different paths points. The origin path is seen as the
/// 'link's location while the dest path is seen as the link's pointed place.
#[derive(Debug)]
struct LinkedPoint {
    origin: PathBuf,
    dest: PathBuf,
}

impl LinkedPoint {
    /// Creates a link representation of two different locations.
    pub(self) fn new(origin: &Path, dest: &Path) -> Self {
        Self {
            origin: origin.into(),
            dest: dest.into(),
        }
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest.
    pub(self) fn synced(&self) -> bool {
        if self.origin.exists() && self.dest.exists() {
            if let Some(linked) = modified(&self.dest) {
                if let Some(link) = modified(&self.origin) {
                    return link >= linked;
                }
            }
        }

        false
    }

    /// Syncs (or Links) the two points on the filesystem. The behaviour of this function
    /// for making the sync is controlled by the overwrite option. See the docs for OverwriteMode
    /// to get more info.
    pub(self) fn mirror(&self, overwrite: OverwriteMode) -> Result<()> {
        if overwrite == OverwriteMode::Disallow && self.origin.exists() {
            err!(AppErrorType::ObjectExists(
                self.origin.display().to_string()
            ));
        }

        if overwrite == OverwriteMode::Force || overwrite == OverwriteMode::Allow && !self.synced()
        {
            fs::copy(&self.dest, &self.origin).context("Unable to copy the file")?;
            info!(
                "synced: {} -> {}",
                pathlight(&self.dest),
                pathlight(&self.origin)
            );
        }

        Ok(())
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

/// Represents the different types a path can take on the file system. It is just a convenience
/// enum for using a match instead of an if-else tree.
#[derive(Debug)]
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
