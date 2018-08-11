use failure::ResultExt;

use std::path::PathBuf;

use super::{Link, LinkPoints, LinkTree};
use logger::highlight;
use Result;

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

pub fn update(tree: &mut LinkTree) -> Result<()> {
    debug!(
        "Working on: {} - {}",
        highlight(tree.link.display()),
        highlight(tree.dest.display())
    );
    if !tree.linked() {
        tree.create(LinkPoints::Src)
            .context("Unable to create backup dir")?;
    }

    for entry in tree.read(LinkPoints::Dest).context("Unable to read dir")? {
        match entry {
            Ok(component) => {
                tree.branch(&component.file_name());

                match FileSystemType::new(&tree.dest) {
                    FileSystemType::File => if backup(&tree.link()).is_err() {
                        warn!("Unable to copy {}", highlight(tree.display()));
                    },

                    FileSystemType::Dir => if update(tree).is_err() {
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

fn backup(link: &Link) -> Result<()> {
    if !link.same_points() {
        match link.copy() {
            Ok(()) => {
                info!(
                    "copied: {} -> {}",
                    highlight(link.link.display()),
                    highlight(link.dest.display())
                );
                Ok(())
            }

            Err(err) => Err(err),
        }
    } else {
        info!("Copy not needed for: {}", highlight(link.link.display()));
        Ok(())
    }
}
