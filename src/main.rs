#![allow(deprecated)]

#[macro_use]
extern crate clap;
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate backup_rs as app;

use app::logger::{self, pathlight};
use app::{BackupOptions, ConfigFile, RestoreOptions, Result};
use clap::ArgMatches;
use failure::Fail;
use libc::EXIT_FAILURE;
use std::process::exit;

macro_rules! curr_dir {
    () => {
        std::env::current_dir().expect("Unable to retrieve current work directory")
    };
}

fn main() {
    if logger::init("info").is_err() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    let yaml = load_yaml!("cli.yml");
    let app = App::new(clap::App::from(yaml).get_matches());

    if let Err(err) = app.run() {
        if app.backtrace {
            for cause in err.causes() {
                error!("{}", cause);
            }
        } else {
            error!("{}", err);
        }

        exit(EXIT_FAILURE);
    }
}

struct App<'a> {
    matches: ArgMatches<'a>,
    backtrace: bool,
}

impl<'a> App<'a> {
    pub fn new(matches: ArgMatches<'a>) -> Self {
        let backtrace = matches.is_present("backtrace");

        App { matches, backtrace }
    }

    pub fn run(&self) -> Result<()> {
        match self.matches.subcommand() {
            ("update", Some(matches)) => Backup::new(matches).execute()?,
            ("restore", Some(matches)) => Restore::new(matches).execute()?,
            _ => unreachable!(),
        }

        Ok(())
    }
}

struct Backup<'a> {
    options: BackupOptions,
    path: Option<&'a str>,
}

impl<'a> Backup<'a> {
    pub fn new(matches: &'a ArgMatches<'a>) -> Self {
        let options = BackupOptions::new(matches.is_present("warn"));
        let path = matches.value_of("path");

        Backup { options, path }
    }

    pub fn execute(&self) -> Result<()> {
        let mut path = curr_dir!();

        if let Some(val) = self.path {
            path.push(val);
            debug!("Working directory set to {}", pathlight(&path));
        }

        info!("Starting backup on {}", path.display());
        ConfigFile::load(&path)?.backup(self.options)
    }
}

struct Restore<'a> {
    options: RestoreOptions,
    path: Option<&'a str>,
}

impl<'a> Restore<'a> {
    pub fn new(matches: &'a ArgMatches<'a>) -> Self {
        let options =
            RestoreOptions::new(matches.is_present("warn"), matches.is_present("overwrite"));
        let path = matches.value_of("path");

        Restore { options, path }
    }

    pub fn execute(&self) -> Result<()> {
        let mut path = curr_dir!();

        if let Some(val) = self.path {
            path.push(val);
            debug!("Working directory set to {}", pathlight(&path));
        }

        info!("Starting restore of {}", path.display());
        ConfigFile::load(&path)?.restore(self.options)
    }
}
