use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::ops::restore::{self, ArchiveOptions};
use artid::prelude::*;
use chrono::Utc;
use clap::ArgMatches;
use log::{info, log};
use logger::pathlight;
use std::io;
use std::path::{Path, PathBuf};

use crate::errors::{Error, ErrorKind};
use crate::AppResult;

#[derive(Debug)]
pub struct Restore {
    run: bool,
    overwrite: bool,
    path: PathBuf,
    folder: Option<String>,
    point: Option<usize>,
}

impl Restore {
    pub fn build(matches: &ArgMatches<'_>) -> AppResult<Self> {
        Ok(Self {
            run: !matches.is_present("dry-run"),
            overwrite: matches.is_present("overwrite"),
            path: Self::build_path(matches),
            folder: match matches.value_of("folder") {
                Some(val) => Some(val.into()),
                None => None,
            },
            point: Self::build_point(matches)?,
        })
    }

    pub fn run(&self) -> AppResult<()> {
        info!(
            "Starting restore of the contents in {}",
            pathlight(&self.path)
        );

        let mut archive = ArtidArchive::load(&self.path)?;
        let mut options = ArchiveOptions::new(self.overwrite);

        if let Some(ref value) = self.folder {
            let id = archive.get_folder_id(value).ok_or_else::<Error, _>(|| {
                ErrorKind::InvalidInput {
                    arg: "--folder".to_string(),
                    value: value.to_string(),
                }
                .into()
            })?;

            options = options.with_folders(vec![id.clone()]);

            if let Some(value) = self.point {
                options = options.with_snapshot(
                    archive
                        .history()
                        .iter()
                        .filter(|snapshot| snapshot.contains(&id))
                        .nth(value)
                        .ok_or_else::<Error, _>(|| {
                            ErrorKind::InvalidInput {
                                arg: "--point".to_string(),
                                value: value.to_string(),
                            }
                            .into()
                        })?
                        .timestamp(),
                )
            }
        }

        let model = restore::restore(&mut archive, options)?;
        operate(self.run, model)?;
        archive.save()?;
        Ok(())
    }

    fn build_path(matches: &ArgMatches<'_>) -> PathBuf {
        let mut path = curr_dir!();
        if let Some(val) = matches.value_of("path") {
            path.push(val);
        }

        path
    }

    fn build_point(matches: &ArgMatches<'_>) -> AppResult<Option<usize>> {
        Ok(match matches.value_of("from") {
            Some(val) => match val.parse::<usize>() {
                Ok(value) => Some(value),
                Err(_) => {
                    return Err(Error::new(ErrorKind::InvalidInput {
                        arg: "from".to_string(),
                        value: val.to_string(),
                    }));
                }
            },
            None => None,
        })
    }
}

fn operate<M>(run: bool, model: M) -> AppResult<()>
where
    M: Model<Action = restore::Action, Error = artid::ops::Error>,
{
    if run {
        model.run()?;
        info!("Restore performed successfully");
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
