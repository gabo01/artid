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
pub enum LinkPoints {
    Src,
    Dest,
}

pub struct LinkTree {
    link: PathBuf,
    dest: PathBuf,
    nested_level: u32,
}

impl LinkTree {
    pub fn new<T: AsRef<Path>>(folder: &Folder, path: T) -> Self {
        let link = path.as_ref().join(folder.path.as_ref());
        let dest = PathBuf::from(folder.origin.as_ref());

        Self {
            link,
            dest,
            nested_level: 0,
        }
    }

    pub fn linked(&self) -> bool {
        self.link.is_dir() && self.dest.is_dir()
    }

    pub fn branch<T: AsRef<Path>>(&mut self, branch: &T) {
        self.link.push(&branch);
        self.dest.push(&branch);
        self.nested_level += 1;
    }

    pub fn root(&mut self) {
        if self.nested_level == 0 {
            panic!("Can not get the root of the tree root");
        }
        self.link.pop();
        self.dest.pop();
        self.nested_level -= 1;
    }

    pub fn link(&self) -> Link<'_> {
        Link::new(&self.link, &self.dest)
    }

    pub fn create(&self, point: LinkPoints) -> std::io::Result<()> {
        match point {
            LinkPoints::Src => fs::create_dir_all(&self.link),
            LinkPoints::Dest => fs::create_dir_all(&self.dest),
        }
    }

    pub fn read(&self, point: LinkPoints) -> std::io::Result<ReadDir> {
        match point {
            LinkPoints::Src => fs::read_dir(&self.link),
            LinkPoints::Dest => fs::read_dir(&self.dest),
        }
    }

    pub fn display(&self) -> path::Display {
        self.dest.display()
    }
}

pub struct Link<'a> {
    pub link: &'a Path,
    pub dest: &'a Path,
}

impl<'a> Link<'a> {
    pub fn new(link: &'a Path, dest: &'a Path) -> Self {
        Self { link, dest }
    }

    pub fn copy(&self) -> Result<()> {
        fs::copy(self.dest, self.link).context("Unable to copy the file")?;
        Ok(())
    }

    pub fn same_points(&self) -> bool {
        if !self.link.exists() {
            return false;
        }

        if let Some(time_src) = modified(self.dest) {
            if let Some(time_dest) = modified(self.link) {
                return time_dest >= time_src;
            }
        }

        false
    }
}

fn modified<T: AsRef<Path>>(file: T) -> Option<SystemTime> {
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
