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
        folder: PathBuf,
    },

    Restore {
        run: bool,
        overwrite: bool,
        folder: PathBuf,
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
            folder: {
                let mut path = curr_dir!();
                if let Some(val) = matches.value_of("path") {
                    path.push(val);
                }

                path
            },
        }
    }

    fn build_restore(matches: &ArgMatches<'_>) -> Self {
        Operation::Restore {
            run: !matches.is_present("dry-run"),
            overwrite: matches.is_present("overwrite"),
            folder: {
                let mut path = curr_dir!();
                if let Some(val) = matches.value_of("path") {
                    path.push(val);
                }

                path
            },
        }
    }

    fn run(&self) -> AppResult<()> {
        match *self {
            Operation::Backup { run, ref folder } => {
                backup(run, folder)?;
            }

            Operation::Restore {
                run,
                overwrite,
                ref folder,
            } => {
                restore(run, overwrite, folder)?;
            }
        }

        Ok(())
    }
}

fn backup(run: bool, folder: &Path) -> AppResult<()> {
    info!("Starting backup on {}", pathlight(folder));

    let options = BackupOptions::new(run);
    let stamp = ConfigFile::load(folder)?.backup(options)?;

    if !run {
        info!(
            "Bakup timestamp in {}",
            highlight(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
        );
    }

    Ok(())
}

fn restore(run: bool, overwrite: bool, folder: &Path) -> AppResult<()> {
    info!("Starting restore of the contents in {}", pathlight(folder));

    let options = RestoreOptions::new(run, overwrite);
    ConfigFile::load(folder)?.restore(options)?;

    Ok(())
}
