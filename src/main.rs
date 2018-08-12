#![allow(deprecated)]

#[macro_use]
extern crate clap;
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate backup_rs as app;

use app::logger::{self, pathlight};
use app::{ConfigFile, Result};
use clap::ArgMatches;
use failure::Fail;
use libc::EXIT_FAILURE;
use std::process::exit;

fn main() {
    if logger::init("info").is_err() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    let yaml = load_yaml!("cli.yml");
    let app = App::new(clap::App::from(yaml).get_matches());
    let backtrace = app.matches.is_present("backtrace");

    if let Err(err) = app.run() {
        if backtrace {
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
}

impl<'a> App<'a> {
    pub fn new(matches: ArgMatches<'a>) -> Self {
        App { matches }
    }

    pub fn run(self) -> Result<()> {
        match self.matches.subcommand() {
            ("update", Some(matches)) => Backup::new(matches).execute()?,
            ("restore", Some(matches)) => Restore::new(matches).execute()?,
            _ => unreachable!(),
        }

        Ok(())
    }
}

struct Backup<'a, 'b>
where
    'b: 'a,
{
    matches: &'a ArgMatches<'b>,
}

impl<'a, 'b> Backup<'a, 'b> {
    pub fn new(matches: &'a ArgMatches<'b>) -> Self {
        Backup { matches }
    }

    pub fn execute(&self) -> Result<()> {
        let mut path = std::env::current_dir().expect("Unable to retrieve current work directory");

        if let Some(val) = self.matches.value_of("path") {
            path.push(val);
            debug!("Working directory set to {}", pathlight(&path));
        }

        info!("Starting backup on {}", path.display());
        ConfigFile::load(&path)?.backup(&path)
    }
}

struct Restore<'a, 'b>
where
    'b: 'a,
{
    matches: &'a ArgMatches<'b>,
}

impl<'a, 'b> Restore<'a, 'b> {
    pub fn new(matches: &'a ArgMatches<'b>) -> Self {
        Restore { matches }
    }

    pub fn execute(&self) -> Result<()> {
        let mut path = std::env::current_dir().expect("Unable to retrieve current work directory");

        if let Some(val) = self.matches.value_of("path") {
            path.push(val);
            debug!("Working directory set to {}", pathlight(&path));
        }

        info!("Starting restore of {}", path.display());
        ConfigFile::load(&path)?.restore(&path)
    }
}
