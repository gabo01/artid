use chrono::Utc;
use failure::ResultExt;
use log::{info, log};
use std::io;
use std::path::Path;

use crate::{AppError, AppResult, ErrorType};
use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::ops::restore::{self, ArchiveOptions};
use artid::prelude::*;
use logger::pathlight;

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
        let id = archive.get_folder_id(value).ok_or_else(|| {
            AppError::from(ErrorType::BadArgument(
                value.to_string(),
                "--folder".to_string(),
            ))
        })?;

        options = options.with_folders(vec![id.clone()]);

        if let Some(value) = point.to_owned() {
            options = options.with_snapshot(
                archive
                    .history()
                    .iter()
                    .filter(|snapshot| snapshot.contains(&id))
                    .nth(value)
                    .ok_or_else(|| {
                        AppError::from(ErrorType::BadArgument(
                            value.to_string(),
                            "--point".to_string(),
                        ))
                    })?
                    .timestamp(),
            )
        }
    }

    let model = restore::restore(&mut archive, options).context(ErrorType::Operative)?;
    operate(run, model)?;
    archive.save()?;
    Ok(())
}

fn operate<M>(run: bool, model: M) -> AppResult<()>
where
    M: Model<Action = restore::Action, Error = io::Error>,
{
    if run {
        model.run().context(ErrorType::Operative)?;
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
