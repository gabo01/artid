#![allow(deprecated)]

#[macro_use]
extern crate clap;
extern crate chrono;
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate artid as app;

use app::logger::{self, highlight, pathlight};
use app::{BackupOptions, ConfigFile, RestoreOptions, Result};
use chrono::{offset::Utc, DateTime, SecondsFormat};
use clap::{App, ArgMatches};
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
    let matches = App::from_yaml(yaml)
        .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .get_matches();

    if let Err(err) = run(&matches) {
        if matches.is_present("backtrace") {
            for cause in err.causes() {
                error!("{}", cause);
            }
        } else {
            error!("{}", err);
        }

        exit(EXIT_FAILURE);
    }
}

fn run(matches: &ArgMatches<'_>) -> Result<()> {
    match matches.subcommand_name() {
        Some(e @ "backup") => {
            let stamp = backup(matches.subcommand_matches(e).unwrap())?;
            if !matches
                .subcommand_matches(e)
                .unwrap()
                .is_present("dry-run")
            {
                info!(
                    "Bakup timestamp in {}",
                    highlight(stamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
                );
            }
        }

        Some(e @ "restore") => restore(matches.subcommand_matches(e).unwrap())?,
        _ => unreachable!()
    }

    Ok(())
}

fn backup(matches: &ArgMatches) -> Result<DateTime<Utc>> {
    let options = BackupOptions::new(!matches.is_present("dry-run"));
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
