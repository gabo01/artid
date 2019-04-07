//! This file contains the command line parser wrapper.
//!
//! The wrappers job is to call the command line parser and create a model of the operations
//! that the user wishes to perform.
use artid::prelude::*;
use chrono::{SecondsFormat, Utc};
use clap::{crate_authors, crate_description, crate_version, load_yaml, App, ArgMatches};
use logger::{highlight, pathlight};
use std::path::{Path, PathBuf};

use super::ops;
use crate::errors::{Error, ErrorKind};
use crate::AppResult;

pub fn parse() -> AppResult<AppInfo> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml)
        .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .get_matches();

    AppInfo::new(&matches)
}

#[derive(Debug)]
pub struct AppInfo {
    trace: bool,
    operation: Operation,
}

impl AppInfo {
    fn new(matches: &ArgMatches<'_>) -> AppResult<Self> {
        let trace = matches.is_present("backtrace");
        let operation = Operation::build(&matches)?;

        Ok(Self { trace, operation })
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
    Backup(ops::Backup),
    Restore(ops::Restore),
}

impl Operation {
    fn build(matches: &ArgMatches<'_>) -> AppResult<Operation> {
        Ok(match matches.subcommand_name() {
            Some(command) => {
                // If a subcommand exists, it's matches must also exist
                let matches = matches.subcommand_matches(command).unwrap();

                match command {
                    "backup" => Operation::Backup(ops::Backup::build(matches)),
                    "restore" => Operation::Restore(ops::Restore::build(matches)?),
                    _ => unreachable!(),
                }
            }

            None => unreachable!(),
        })
    }

    fn run(&self) -> AppResult<()> {
        match *self {
            Operation::Backup(ref op) => op.run(),
            Operation::Restore(ref op) => op.run(),
        }
    }
}
