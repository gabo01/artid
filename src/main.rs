#![allow(deprecated)]

#[macro_use]
extern crate clap;
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;

extern crate backup_rs as app;

use clap::ArgMatches;
use failure::Fail;
use libc::EXIT_FAILURE;

use std::path::PathBuf;
use std::process::exit;

use app::logger::{self, pathlight};
use app::{ConfigFile, Result};

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
    path: PathBuf,
}

impl<'a> App<'a> {
    pub fn new(matches: ArgMatches<'a>) -> Self {
        App {
            matches,
            path: std::env::current_dir().expect("Unable to retrieve the current dir"),
        }
    }

    pub fn run(mut self) -> Result<()> {
        if let Some(val) = self.matches.value_of("dir") {
            self.path.push(val);
            debug!(
                "Working directory set to {}",
                pathlight(&self.path)
            );
        }

        if let Some(val) = self.matches.subcommand_name() {
            match val {
                "update" => Backup::new(&self).execute()?,
                _ => unreachable!(),
            }
        }

        Ok(())
    }
}

struct Backup<'a, 'b>
where
    'b: 'a,
{
    app: &'a App<'b>,
}

impl<'a, 'b> Backup<'a, 'b> {
    pub fn new(app: &'a App<'b>) -> Self {
        Backup { app }
    }

    pub fn execute(&self) -> Result<()> {
        info!("Starting backup on {}", self.app.path.display());
        ConfigFile::load(&self.app.path)?.backup(&self.app.path)
    }
}
