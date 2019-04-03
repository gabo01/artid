use artid::ops::backup::{self, ArchiveOptions};
use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::prelude::*;
use chrono::Utc;
use log::{info, log};
use logger::pathlight;
use std::io;
use std::path::{Path, PathBuf};
use clap::{ArgMatches};

use crate::errors::{Error, ErrorKind};
use crate::AppResult;

#[derive(Debug)]
pub struct Backup {
    run: bool,
    path: PathBuf,
    folder: Option<String>,
}

impl Backup {
    pub fn build(matches: &ArgMatches<'_>) -> Self {
        Self {
            run: !matches.is_present("dry-run"),
            path: {
                let mut path = curr_dir!();
                if let Some(val) = matches.value_of("path") {
                    path.push(val);
                }

                path
            },

            folder: match matches.value_of("folder") {
                Some(val) => Some(val.into()),
                None => None,
            },
        }
    }

    pub fn run(&self) -> AppResult<()> {
        info!("Starting backup on {}", pathlight(&self.path));

        let mut archive = ArtidArchive::load(&self.path)?;
        let options = match self.folder {
            Some(ref value) => ArchiveOptions::with_folders(vec![archive
                .get_folder_id(value)
                .ok_or_else::<Error, _>(|| {
                    ErrorKind::InvalidInput {
                        arg: "--folder".to_string(),
                        value: value.to_string(),
                    }
                    .into()
                })?]),

            None => ArchiveOptions::default(),
        };

        let model = backup::backup(&mut archive, options)?;
        operate(self.run, model)?;
        archive.save()?;
        Ok(())
    }
}

fn operate<M>(run: bool, model: M) -> AppResult<()>
where
    M: Model<Action = backup::Action, Error = artid::ops::Error>,
{
    if run {
        model.run()?;
        info!("Backup performed successfully");
    } else {
        model.log(&|action| {
            if let CopyAction::CopyFile { ref src, ref dst } = action {
                info!(
                    "sync {} -> {}",
                    pathlight(src.path()),
                    pathlight(dst.path())
                );
            }
        });
    }

    Ok(())
}
