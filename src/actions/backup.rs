use super::{highlight, AppError, AppErrorType, ConfigFile, Folder, Result, ResultExt};
use std::fs;
use std::path::{self, Path, PathBuf};
use std::time::SystemTime;

const MIN_CAPACITY: usize = 15;

pub struct Backup<T: AsRef<Path>> {
    path: T,
}

impl<T: AsRef<Path>> Backup<T> {
    pub fn new(path: T) -> Self {
        Backup { path }
    }

    pub fn execute(&self) -> Result<()> {
        let file = ConfigFile::load(&self.path)?;

        for folder in file {
            let mut tree = DirTree::new(dest(&self.path.as_ref(), &folder)?, src(&folder)?);
            Self::update(&mut tree).context(AppErrorType::UpdateFolder(format!(
                "{}",
                self.path.as_ref().display()
            )))?;
        }

        Ok(())
    }

    fn update(tree: &mut DirTree) -> Result<()> {
        fs::create_dir_all(tree.dest()).context("Unable to create backup dir")?;

        for entry in fs::read_dir(tree.src().unwrap()).context("Unable to read dir")? {
            match entry {
                Ok(component) => {
                    tree.branch(component.path(), component.file_name());

                    match FileSystemType::new(tree.src().unwrap()) {
                        FileSystemType::File => if let Err(_) = backup(tree.link()) {
                            warn!("Unable to copy {}", highlight(tree.display()));
                        },

                        FileSystemType::Dir => if let Err(_) = Self::update(tree) {
                            warn!("Unable to read {}", highlight(tree.display()));
                        },

                        FileSystemType::Other => {
                            warn!("Unable to process {}", highlight(tree.display()));
                        }
                    }

                    tree.root();
                }

                Err(_) => warn!("Unable to read entry"),
            }
        }

        Ok(())
    }
}

struct DirTree {
    root: PathBuf,
    links: Vec<PathBuf>,
}

impl DirTree {
    pub fn new(root: PathBuf, link: PathBuf) -> DirTree {
        let mut links = Vec::with_capacity(MIN_CAPACITY);
        links.push(link);

        DirTree { root, links }
    }

    pub fn src<'a>(&'a self) -> Option<&'a PathBuf> {
        self.links.last()
    }

    pub fn dest(&self) -> &PathBuf {
        &(self.root)
    }

    pub fn branch<P: AsRef<Path>>(&mut self, element: PathBuf, branch: P) {
        self.root.push(branch);
        self.links.push(element);
    }

    pub fn root(&mut self) {
        self.root.pop();
        self.links.pop();
    }

    pub fn link<'a>(&'a self) -> Link<'a> {
        Link::new(self.links.last().unwrap(), &self.root)
    }

    pub fn display(&self) -> path::Display {
        self.links.last().unwrap().display()
    }
}

pub struct Link<'a> {
    src: &'a PathBuf,
    dest: &'a PathBuf,
}

impl<'a> Link<'a> {
    pub fn new(src: &'a PathBuf, dest: &'a PathBuf) -> Link<'a> {
        Link { src, dest }
    }

    pub fn src(&self) -> &PathBuf {
        self.src
    }

    pub fn dest(&self) -> &PathBuf {
        self.dest
    }

    pub fn same_points(&self) -> bool {
        if !self.dest.exists() {
            return false;
        }

        if let Some(time_src) = modified(self.src) {
            if let Some(time_dest) = modified(self.dest) {
                return time_dest >= time_src;
            }
        }

        false
    }

    pub fn copy(&self) -> Result<()> {
        fs::copy(self.src, self.dest).context("Unable to copy the file")?;
        Ok(())
    }
}

fn modified(file: &PathBuf) -> Option<SystemTime> {
    match file.metadata() {
        Ok(data) => match data.modified() {
            Ok(time) => Some(time),
            Err(_) => {
                warn!(
                    "Unable to access modified attribute of {}",
                    highlight(file.display())
                );
                None
            }
        },

        Err(_) => {
            warn!("Unable to access {} metadata", highlight(file.display()));
            None
        }
    }
}

pub fn src(folder: &Folder) -> Result<PathBuf> {
    let src = folder.origin.path().to_owned();

    if !src.is_dir() {
        Err(AppError::from(AppErrorType::NotDir(format!(
            "path {} is not a dir",
            highlight(src.display())
        ))))
    } else {
        Ok(src)
    }
}

pub fn dest<T: AsRef<Path>>(path: &T, folder: &Folder) -> Result<PathBuf> {
    let dest = path.as_ref().join(&folder.path);

    if !dest.is_dir() {
        Err(AppError::from(AppErrorType::NotDir(format!(
            "{}",
            highlight(dest.display())
        ))))
    } else {
        Ok(dest)
    }
}

fn backup(link: Link) -> Result<()> {
    if !link.same_points() {
        match link.copy() {
            Ok(()) => {
                info!(
                    "copied: {} -> {}",
                    highlight(link.src().display()),
                    highlight(link.dest().display())
                );
                Ok(())
            }

            Err(err) => Err(err),
        }
    } else {
        info!("Copy not needed for: {}", highlight(link.src().display()));
        Ok(())
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
