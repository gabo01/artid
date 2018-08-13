use failure::{Fail, ResultExt};
use logger::{highlight, pathlight};
use std;
use std::fs::{self, ReadDir};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use Result;

/// Represents the pieces that make a linked, the link itself and the place where it's pointing
#[derive(Copy, Clone)]
pub enum LinkPiece {
    Link,
    Linked,
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
pub struct LinkTree {
    origin: PathBuf,
    dest: PathBuf,
    deepness: u32,
}

impl LinkTree {
    /// Creates a new link representation of two different trees
    pub fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self {
            origin,
            dest,
            deepness: 0,
        }
    }

    /// Checks if the two points are linked. Two points are linked if they are both directories
    pub fn linked(&self) -> bool {
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

    /// Creates the link piece specified by point
    pub fn create(&self, point: LinkPiece) -> std::io::Result<()> {
        match point {
            LinkPiece::Link => fs::create_dir_all(&self.origin),
            LinkPiece::Linked => fs::create_dir_all(&self.dest),
        }
    }

    /// Reads the link piece specified by point
    pub fn read(&self, point: LinkPiece) -> std::io::Result<ReadDir> {
        match point {
            LinkPiece::Link => fs::read_dir(&self.origin),
            LinkPiece::Linked => fs::read_dir(&self.dest),
        }
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the destination dir, the 'link' or if it is unable to
    /// read the contents of the origin, the 'linked', dir.
    /// 
    /// If there is an error while processing a subcomponent the function will emit a warning
    /// but will try to finish the work anyway. In debug mode, the function will print the trace
    /// of the warnings if finds
    pub fn sync(&mut self) -> Result<()> {
        debug!(
            "Syncing {} with {}",
            pathlight(&self.dest),
            pathlight(&self.origin)
        );

        if !self.linked() {
            self.create(LinkPiece::Link)
                .context("Unable to create backup dir")?;
        }

        for entry in self.read(LinkPiece::Linked).context("Unable to read dir")? {
            match entry {
                Ok(component) => {
                    self.branch(&component.file_name());

                    match FileSystemType::new(&self.dest) {
                        FileSystemType::File => if let Err(err) = self.link().mirror() {
                            warn!("Unable to copy {}", pathlight(&self.dest));
                            if cfg!(debug_assertions) {
                                for cause in err.causes() {
                                    trace!("{}", cause);
                                }
                            }
                        },

                        FileSystemType::Dir => if let Err(err) = self.sync() {
                            warn!("Unable to read {}", pathlight(&self.dest));
                            if cfg!(debug_assertions) {
                                for cause in err.causes() {
                                    trace!("{}", cause);
                                }
                            }
                        },

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
pub struct LinkedPoint<'a> {
    pub origin: &'a Path,
    pub dest: &'a Path,
}

impl<'a> LinkedPoint<'a> {
    /// Creates a link representation of two different locations
    pub fn new(origin: &'a Path, dest: &'a Path) -> Self {
        Self { origin, dest }
    }

    /// Links the two points in the filesystem. This implies making a copy of the object in
    /// dest to origin 
    pub fn link(&self) -> Result<()> {
        fs::copy(self.dest, self.origin).context("Unable to copy the file")?;
        Ok(())
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest
    pub fn linked(&self) -> bool {
        if !self.origin.exists() {
            return false;
        }

        if let Some(linked) = get_last_modified(self.dest) {
            if let Some(link) = get_last_modified(self.origin) {
                return link >= linked;
            }
        }

        false
    }

    /// Links the two points on the filesystem. This method will check first if the two
    /// objects aren't already linked before making a link. In order words, the .link()
    /// method will make a forced link of the two points while this method will link the
    /// points only if necessary.chrono
    pub fn mirror(&self) -> Result<()> {
        if !self.linked() {
            match self.link() {
                Ok(()) => {
                    info!(
                        "synced: {} -> {}",
                        pathlight(self.dest),
                        pathlight(self.origin)
                    );
                    Ok(())
                }

                Err(err) => Err(err),
            }
        } else {
            info!("Copy not needed for: {}", pathlight(self.dest));
            Ok(())
        }
    }
}

/// Queries the filesystem and gets the date of the last time the file was modified keeped
/// by the system. Since this is a measurement made by the system, the time returned by this
/// function can be wrong in some cases: the user changed the date in it's system, an operation
/// was queued and performed at a later time and some other cases.
fn get_last_modified<P: AsRef<Path>>(file: P) -> Option<SystemTime> {
    match file.as_ref().metadata() {
        Ok(data) => match data.modified() {
            Ok(time) => Some(time),
            Err(_) => {
                warn!(
                    "Unable to access modified attribute of {}",
                    highlight(file.as_ref().display())
                );
                None
            }
        },

        Err(_) => {
            warn!(
                "Unable to access {} metadata",
                highlight(file.as_ref().display())
            );
            None
        }
    }
}

/// Represents the different types a path can take on the file system
enum FileSystemType {
    File,
    Dir,
    Other,
}

impl FileSystemType {
    fn new(obj: &PathBuf) -> FileSystemType {
        if obj.is_file() {
            FileSystemType::File
        } else if obj.is_dir() {
            FileSystemType::Dir
        } else {
            FileSystemType::Other
        }
    }
}
