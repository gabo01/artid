use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::ops::restore::{self, ArchiveOptions};
use artid::prelude::*;
use chrono::Utc;
use log::{info, log};
use logger::pathlight;
use std::io;
use std::path::Path;

use crate::errors::{Error, ErrorKind};
use crate::AppResult;

pub fn restore(
    run: bool,
    overwrite: bool,
    path: &Path,
    folder: &Option<String>,
    point: &Option<usize>,
) -> AppResult<()> {
    info!("Starting restore of the contents in {}", pathlight(path));

    let mut archive = ArtidArchive::load(path)?;
    let mut options = ArchiveOptions::new(overwrite);

    if let Some(ref value) = folder {
        let id = archive.get_folder_id(value).ok_or_else::<Error, _>(|| {
            ErrorKind::InvalidInput {
                arg: "--folder".to_string(),
                value: value.to_string(),
            }
            .into()
        })?;

        options = options.with_folders(vec![id.clone()]);

        if let Some(value) = point.to_owned() {
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
    operate(run, model)?;
    archive.save()?;
    Ok(())
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
