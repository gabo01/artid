use failure::{Fail, ResultExt};
use logger::{highlight, pathlight};
use std;
use std::fs::{self, ReadDir};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use Result;

#[derive(Copy, Clone)]
pub enum LinkPiece {
    Link,
    Linked,
}

pub struct LinkTree {
    origin: PathBuf,
    dest: PathBuf,
    deepness: u32,
}

impl LinkTree {
    pub fn new(origin: PathBuf, dest: PathBuf) -> Self {
        Self {
            origin,
            dest,
            deepness: 0,
        }
    }

    pub fn linked(&self) -> bool {
        self.origin.is_dir() && self.dest.is_dir()
    }

    pub fn branch<P: AsRef<Path>>(&mut self, branch: &P) {
        self.origin.push(&branch);
        self.dest.push(&branch);
        self.deepness += 1;
    }

    pub fn root(&mut self) {
        if self.deepness == 0 {
            panic!("Can not get the root of the tree root");
        }
        self.origin.pop();
        self.dest.pop();
        self.deepness -= 1;
    }

    pub fn link(&self) -> LinkedPoint<'_> {
        LinkedPoint::new(&self.origin, &self.dest)
    }

    pub fn create(&self, point: LinkPiece) -> std::io::Result<()> {
        match point {
            LinkPiece::Link => fs::create_dir_all(&self.origin),
            LinkPiece::Linked => fs::create_dir_all(&self.dest),
        }
    }

    pub fn read(&self, point: LinkPiece) -> std::io::Result<ReadDir> {
        match point {
            LinkPiece::Link => fs::read_dir(&self.origin),
            LinkPiece::Linked => fs::read_dir(&self.dest),
        }
    }

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

pub struct LinkedPoint<'a> {
    pub origin: &'a Path,
    pub dest: &'a Path,
}

impl<'a> LinkedPoint<'a> {
    pub fn new(origin: &'a Path, dest: &'a Path) -> Self {
        Self { origin, dest }
    }

    pub fn link(&self) -> Result<()> {
        fs::copy(self.dest, self.origin).context("Unable to copy the file")?;
        Ok(())
    }

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
