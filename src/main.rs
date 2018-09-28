#![allow(deprecated)]

#[macro_use]
extern crate clap;
extern crate chrono;
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate backup_rs as app;

use app::logger::{self, highlight, pathlight};
use app::{BackupOptions, ConfigFile, RestoreOptions, Result};
use chrono::{offset::Utc, DateTime, SecondsFormat};
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
    let mut app = App::new(clap::App::from(yaml));

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
    app: clap::App<'a, 'a>,
    matches: ArgMatches<'a>,
    backtrace: bool,
}

impl<'a> App<'a> {
    pub fn new(app: clap::App<'a, 'a>) -> Self {
        let matches = app.clone().get_matches();
        let backtrace = matches.is_present("backtrace");

        App {
            app,
            matches,
            backtrace,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        match self.matches.subcommand_name() {
            Some("update") => {
                let stamp = backup(self.matches.subcommand_matches("update").unwrap())?;
                if !self
                    .matches
                    .subcommand_matches("update")
                    .unwrap()
                    .is_present("dry-run")
                {
                    info!(
                        "Bakup timestamp in {}",
                        highlight(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
                    );
                }
            }

            Some("restore") => restore(self.matches.subcommand_matches("restore").unwrap())?,
            _ => {
                self.app.print_long_help().unwrap();
                println!();
            }
        }

        Ok(())
    }
}

fn backup(matches: &ArgMatches) -> Result<DateTime<Utc>> {
    let options = BackupOptions::new(matches.is_present("warn"), !matches.is_present("dry-run"));
    let mut path = curr_dir!();

    if let Some(val) = matches.value_of("path") {
        path.push(val);
        debug!("Working directory set to {}", pathlight(&path));
    }

    info!("Starting backup on {}", path.display());
    ConfigFile::load(&path)?.backup(options)
}

fn restore(matches: &ArgMatches) -> Result<()> {
    let options = RestoreOptions::new(
        matches.is_present("warn"),
        matches.is_present("overwrite"),
        !matches.is_present("dry-run"),
    );
    let mut path = curr_dir!();

    if let Some(val) = matches.value_of("path") {
        path.push(val);
        debug!("Working directory set to {}", pathlight(&path));
    }

    info!("Starting restore of {}", path.display());
    ConfigFile::load(&path)?.restore(options)
}
