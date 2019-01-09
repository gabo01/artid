use chrono::Utc;
use failure::ResultExt;
use std::io;
use std::path::Path;

use app::ops::backup::{self, Options};
use app::ops::core::CopyAction;
use app::prelude::*;
use logger::pathlight;
use {AppError, AppResult, ErrorType};

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

fn operate<M: Model<Action = CopyAction, Error = io::Error>>(run: bool, model: M) -> AppResult<()> {
    if run {
        model.run().context(ErrorType::Operative)?;
        info!("Backup performed successfully");
    } else {
        model.log(&|action| {
            if let CopyAction::CopyFile { ref src, ref dst } = action {
                info!("sync {} -> {}", pathlight(&src), pathlight(&dst));
            }
        });
    }

    Ok(())
}
