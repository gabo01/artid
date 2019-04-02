use artid::ops::backup::{self, ArchiveOptions};
use artid::ops::core::{filesystem::Route, model::CopyAction};
use artid::prelude::*;
use chrono::Utc;
use log::{info, log};
use logger::pathlight;
use std::io;
use std::path::Path;

use crate::errors::{Error, ErrorKind};
use crate::AppResult;

pub fn backup(run: bool, path: &Path, folder: &Option<String>) -> AppResult<()> {
    info!("Starting backup on {}", pathlight(path));

    let mut archive = ArtidArchive::load(path)?;
    let options = match folder {
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
    operate(run, model)?;
    archive.save()?;
    Ok(())
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
