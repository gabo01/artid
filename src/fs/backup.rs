use failure::{Fail, ResultExt};

use std::path::PathBuf;

use super::{LinkPiece, LinkTree};
use logger::pathlight;
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
        "Syncing {} with {}",
        pathlight(&tree.dest),
        pathlight(&tree.origin)
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
                    FileSystemType::File => if let Err(err) = tree.link().mirror() {
                        warn!("Unable to copy {}", pathlight(&tree.dest));
                        if cfg!(debug_assertions) {
                            for cause in err.causes() {
                                trace!("{}", cause);
                            }
                        }
                    },

                    FileSystemType::Dir => if let Err(err) = update(tree) {
                        warn!("Unable to read {}", pathlight(&tree.dest));
                        if cfg!(debug_assertions) {
                            for cause in err.causes() {
                                trace!("{}", cause);
                            }
                        }
                    },

                    FileSystemType::Other => {
                        warn!("Unable to process {}", pathlight(&tree.dest));
                    }
                }

                tree.root();
            }

            Err(_) => warn!("Unable to read entry"),
        }
    }

    Ok(())
}
