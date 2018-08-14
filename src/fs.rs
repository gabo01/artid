use failure::{Fail, ResultExt};
use logger::pathlight;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {AppError, AppErrorType, Result};

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
    /// Controls how to handle if a location to be written on already exists. See OverwriteMode
    /// docs for more info on how this setting behaves.
    pub overwrite: OverwriteMode,
}

impl SyncOptions {
    pub fn new(warn: bool, overwrite: OverwriteMode) -> Self {
        Self { warn, overwrite }
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
    deepness: u32,
}

impl LinkTree {
    /// Creates a new link representation for two different trees
    pub fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self {
            origin,
            dest,
            deepness: 0,
        }
    }

    /// Checks if the tree is valid. The tree is valid if the two points are directories.
    pub fn valid(&self) -> bool {
        self.origin.is_dir() && self.dest.is_dir()
    }

    /// Creates an internal representation of the branch of the tree
    pub fn branch<P: AsRef<Path>>(&mut self, branch: &P) {
        self.origin.push(&branch);
        self.dest.push(&branch);
        self.deepness += 1;
    }

    /// Returns to the root of the currently branch.
    ///
    /// This function will panic if the tree is already at it's uppermost root
    pub fn root(&mut self) {
        if self.deepness == 0 {
            panic!("Can not get the root of the tree root");
        }
        self.origin.pop();
        self.dest.pop();
        self.deepness -= 1;
    }

    /// Creates a link object between the current points in the tree
    pub fn link(&self) -> LinkedPoint<'_> {
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
        let options = options.into();

        debug!(
            "Syncing {} with {}",
            pathlight(&self.dest),
            pathlight(&self.origin)
        );

        if !self.valid() {
            fs::create_dir_all(&self.origin).context("Unable to create backup dir")?;
        }

        for entry in fs::read_dir(&self.dest).context("Unable to read dir")? {
            match entry {
                Ok(component) => {
                    self.branch(&component.file_name());

                    match FileSystemType::from(&self.dest) {
                        FileSystemType::File => {
                            if let Err(err) = self.link().mirror(options.overwrite) {
                                if options.warn {
                                    warn!("Unable to copy {}", pathlight(&self.dest));
                                    if cfg!(debug_assertions) {
                                        for cause in err.causes() {
                                            trace!("{}", cause);
                                        }
                                    }
                                } else {
                                    fail!(err);
                                }
                            }
                        }

                        FileSystemType::Dir => {
                            if let Err(err) = self.sync(options) {
                                if options.warn {
                                    warn!("Unable to read {}", pathlight(&self.dest));
                                    if cfg!(debug_assertions) {
                                        for cause in err.causes() {
                                            trace!("{}", cause);
                                        }
                                    }
                                } else {
                                    fail!(err)
                                }
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

        Ok(())
    }
}

/// Represents a link between two different paths points. The origin path is seen as the
/// 'link's location while the dest path is seen as the link's pointed place.
#[derive(Debug)]
pub struct LinkedPoint<'a> {
    pub origin: &'a Path,
    pub dest: &'a Path,
}

impl<'a> LinkedPoint<'a> {
    /// Creates a link representation of two different locations
    pub fn new(origin: &'a Path, dest: &'a Path) -> Self {
        Self { origin, dest }
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest
    pub fn synced(&self) -> bool {
        if self.origin.exists() && self.dest.exists() {
            if let Some(linked) = modified(self.dest) {
                if let Some(link) = modified(self.origin) {
                    return link >= linked;
                }
            }
        }

        false
    }

    /// Links the two points on the filesystem. This method will check first if the two
    /// objects aren't already linked before making a link. In order words, the .link()
    /// method will make a forced link of the two points while this method will link the
    /// points only if necessary. If overwrite = false, this function will exit with an
    /// error if origin already exists
    pub fn mirror(&self, overwrite: OverwriteMode) -> Result<()> {
        if overwrite == OverwriteMode::Disallow && self.origin.exists() {
            err!(AppErrorType::ObjectExists(
                self.origin.display().to_string()
            ));
        }

        if overwrite == OverwriteMode::Force || overwrite == OverwriteMode::Allow && !self.synced()
        {
            fs::copy(self.dest, self.origin).context("Unable to copy the file")?;
            info!(
                "synced: {} -> {}",
                pathlight(self.dest),
                pathlight(self.origin)
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
/// enum for using a match instead of an if-else tree
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
