use chrono::Utc;
use failure::ResultExt;
use std::io;
use std::path::Path;

use app::ops::core::CopyAction;
use app::ops::restore::{self, Options};
use app::prelude::*;
use logger::pathlight;
use {AppError, AppResult, ErrorType};

pub fn restore(
    run: bool,
    overwrite: bool,
    path: &Path,
    folder: &Option<String>,
    point: &Option<usize>,
) -> AppResult<()> {
    info!("Starting restore of the contents in {}", pathlight(path));
    let options = match point.to_owned() {
        Some(value) => Options::with_point(overwrite, value),

        None => Options::new(overwrite),
    };

    let mut config = ConfigFile::load(path)?;

    match folder {
        Some(ref value) => {
            let mut folder = get_folder(&mut config, value)?;
            let model = restore::restore(&mut folder, options).context(ErrorType::Operative)?;
            operate(run, model)?;
        }

        None => {
            let model = restore::restore(&mut config, options).context(ErrorType::Operative)?;
            operate(run, model)?;
        }
    }

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
        info!("Restore performed successfully");
    } else {
        model.log(&|action| {
            if let CopyAction::CopyFile { ref src, ref dst } = action {
                info!("sync {} -> {}", pathlight(&src), pathlight(&dst));
            }
        });
    }

    Ok(())
}
