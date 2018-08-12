use failure::ResultExt;

use std::path::PathBuf;

use super::{LinkPiece, LinkTree};
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
        highlight(tree.origin.display()),
        highlight(tree.dest.display())
    );
    if !tree.linked() {
        tree.create(LinkPiece::Link)
            .context("Unable to create backup dir")?;
    }

    for entry in tree.read(LinkPiece::Linked).context("Unable to read dir")? {
        match entry {
            Ok(component) => {
                tree.branch(&component.file_name());

                match FileSystemType::new(&tree.dest) {
                    FileSystemType::File => if tree.link().mirror().is_err() {
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
