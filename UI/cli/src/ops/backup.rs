use chrono::Utc;
use failure::ResultExt;
use log::{info, log};
use std::io;
use std::path::Path;

use crate::{AppError, AppResult, ErrorType};
use artid::ops::backup::{self, ArchiveOptions};
use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::prelude::*;
use logger::pathlight;

pub fn backup(run: bool, path: &Path, folder: &Option<String>) -> AppResult<()> {
    info!("Starting backup on {}", pathlight(path));

    let mut archive = ArtidArchive::load(path)?;
    let options = match folder {
        Some(ref value) => {
            ArchiveOptions::with_folders(vec![archive.get_folder_id(value).ok_or_else(|| {
                AppError::from(ErrorType::BadArgument(
                    value.to_string(),
                    "--folder".to_string(),
                ))
            })?])
        }

        None => ArchiveOptions::default(),
    };

    let model = backup::backup(&mut archive, options).context(ErrorType::Operative)?;
    operate(run, model)?;
    archive.save()?;
    Ok(())
}

fn operate<M>(run: bool, model: M) -> AppResult<()>
where
    M: Model<Action = backup::Action, Error = io::Error>,
{
    if run {
        model.run().context(ErrorType::Operative)?;
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
