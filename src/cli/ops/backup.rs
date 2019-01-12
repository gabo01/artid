use chrono::Utc;
use failure::ResultExt;
use log::{info, log};
use std::io;
use std::path::Path;

use crate::{AppError, AppResult, ErrorType};
use artid::ops::backup::{self, Options};
use artid::ops::core::{CopyAction, Route};
use artid::prelude::*;
use logger::pathlight;

pub fn backup(run: bool, path: &Path, folder: &Option<String>) -> AppResult<()> {
    info!("Starting backup on {}", pathlight(path));

    let options = Options::default();
    let mut config = ConfigFile::load(path)?;

    match folder {
        Some(ref value) => {
            let mut folder = get_folder(&mut config, value)?;
            let model = backup::backup(&mut folder, options).context(ErrorType::Operative)?;
            operate(run, model)?;
        }
        None => {
            let model = backup::backup(&mut config, options).context(ErrorType::Operative)?;
            operate(run, model)?;
        }
    };

    config.save()?;
    Ok(())
}

fn get_folder<'a, P>(config: &'a mut ConfigFile<P>, value: &str) -> AppResult<FileSystemFolder<'a>>
where
    P: AsRef<Path> + ::std::fmt::Debug,
{
    config.get_folder(value).ok_or_else(|| {
        AppError::from(ErrorType::BadArgument(
            value.to_string(),
            "--folder".to_string(),
        ))
    })
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
