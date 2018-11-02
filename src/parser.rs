/// This file contains the command line parser wrapper.
///
/// The wrappers job is to call the command line parser and create a model of the operations
/// that the user wishes to perform.
use chrono::SecondsFormat;
use clap::ArgMatches;
use std::path::{Path, PathBuf};

// Internal imports
use app::logger::{highlight, pathlight};
use app::prelude::*;
use errors::AppError;

macro_rules! curr_dir {
    () => {
        ::std::env::current_dir().expect("Unable to retrieve current work directory")
    };
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub struct Instance {
    trace: bool,
    operation: Operation,
}

impl Instance {
    pub fn new(matches: &ArgMatches<'_>) -> Instance {
        let trace = matches.is_present("backtrace");
        let operation = Operation::new(&matches);

        Self { trace, operation }
    }

    pub fn run(&self) -> AppResult<()> {
        self.operation.run()
    }

    pub fn backtrace(&self) -> bool {
        self.trace
    }
}

#[derive(Debug)]
enum Operation {
    Backup {
        run: bool,
        path: PathBuf,
        folder: Option<String>,
    },

    Restore {
        run: bool,
        overwrite: bool,
        path: PathBuf,
        folder: Option<String>,
    },
}

impl Operation {
    fn new(matches: &ArgMatches<'_>) -> Operation {
        match matches.subcommand_name() {
            Some(command) => {
                // If a subcommand exists, it's matches must also exist
                let matches = matches.subcommand_matches(command).unwrap();

                match command {
                    "backup" => Self::build_backup(matches),

                    "restore" => Self::build_restore(matches),

                    _ => unreachable!(),
                }
            }

            None => unreachable!(),
        }
    }

    fn build_backup(matches: &ArgMatches<'_>) -> Self {
        Operation::Backup {
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

    fn build_restore(matches: &ArgMatches<'_>) -> Self {
        Operation::Restore {
            run: !matches.is_present("dry-run"),
            overwrite: matches.is_present("overwrite"),
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

    fn run(&self) -> AppResult<()> {
        match *self {
            Operation::Backup {
                run,
                ref path,
                ref folder,
            } => {
                backup(run, path, folder)?;
            }

            Operation::Restore {
                run,
                overwrite,
                ref path,
                ref folder,
            } => {
                restore(run, overwrite, path, folder)?;
            }
        }

        Ok(())
    }
}

fn backup(run: bool, path: &Path, folder: &Option<String>) -> AppResult<()> {
    info!("Starting backup on {}", pathlight(path));

    let options = BackupOptions::new(run);
    let mut config = ConfigFile::load(path)?;

    let stamp = match folder {
        Some(ref value) => config.backup_folder(value, options)?,
        None => config.backup(options)?,
    };

    if !run {
        info!(
            "Bakup timestamp in {}",
            highlight(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
        );
    }

    Ok(())
}

fn restore(run: bool, overwrite: bool, path: &Path, folder: &Option<String>) -> AppResult<()> {
    info!("Starting restore of the contents in {}", pathlight(path));

    let options = RestoreOptions::new(run, overwrite);
    let config = ConfigFile::load(path)?;

    match folder {
        Some(ref value) => config.restore_folder(value, options)?,
        None => config.restore(options)?,
    }

    Ok(())
}
