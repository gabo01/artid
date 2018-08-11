use failure::ResultExt;

use std;
use std::fs::{self, ReadDir};
use std::path::{self, Path, PathBuf};
use std::time::SystemTime;

use {AppError, AppErrorType};
use Folder;
use Result;
use logger::highlight;

mod backup;

pub use self::backup::update as backup;

#[derive(Copy, Clone)]
pub enum LinkPoints { 
    Src,
    Dest
}

pub struct LinkTree {
    link: PathBuf,
    dest: PathBuf
}

impl LinkTree {
    pub fn new<T: AsRef<Path>>(folder: &Folder, path: T) -> Result<Self> {
        let link = path.as_ref().join(folder.path.as_ref());
        let dest = PathBuf::from(folder.origin.as_ref());

        if link.is_dir() && dest.is_dir() {
            Ok (Self {
                link,
                dest,
            })
        } else {
            Err(AppError::from(AppErrorType::NotDir("Link elements were not directories".to_string())))
        }
    }

    pub fn branch<T: AsRef<Path>>(&mut self, branch: &T) {
        self.link.push(&branch);
        self.dest.push(&branch);
    }

    pub fn root(&mut self) {
        self.link.pop();
        self.dest.pop();
    }

    pub fn link(&self) -> Link<'_> {
        Link::new(&self.link, &self.dest)
    }

    pub fn create(&self, point: LinkPoints) -> std::io::Result<()> {
        match point {
            LinkPoints::Src => {
                fs::create_dir_all(&self.link)
            },

            LinkPoints::Dest => {
                fs::create_dir_all(&self.dest)
            }
        }
    }

    pub fn read(&self, point: LinkPoints) -> std::io::Result<ReadDir> {
        match point {
            LinkPoints::Src => {
                fs::read_dir(&self.link)
            },

            LinkPoints::Dest => {
                fs::read_dir(&self.dest)
            }
        }
    }

    pub fn display(&self) -> path::Display {
        self.dest.display()
    }
}

pub struct Link<'a> {
    pub link: &'a Path,
    pub dest: &'a Path
}

impl<'a> Link<'a> {
    pub fn new(link: &'a Path, dest: &'a Path) -> Self {
        Self {
            link,
            dest,
        }
    }

    pub fn copy(&self) -> Result<()> {
        fs::copy(self.link, self.dest).context("Unable to copy the file")?;
        Ok(())
    }

    pub fn same_points(&self) -> bool {
        if !self.dest.exists() {
            return false;
        }

        if let Some(time_src) = modified(self.link) {
            if let Some(time_dest) = modified(self.dest) {
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
            warn!("Unable to access {} metadata", highlight(file.as_ref().display()));
            None
        }
    }
}
