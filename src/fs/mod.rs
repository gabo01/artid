use failure::ResultExt;

use std;
use std::fs::{self, ReadDir};
use std::path::{self, Path, PathBuf};
use std::time::SystemTime;

use logger::highlight;
use {Folder, Result};

mod backup;

pub use self::backup::update as backup;

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
    pub fn new<P: AsRef<Path>>(folder: &Folder, path: P) -> Self {
        let origin = path.as_ref().join(folder.path.as_ref());
        let dest = PathBuf::from(folder.origin.as_ref());

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

    pub fn display(&self) -> path::Display {
        self.dest.display()
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
