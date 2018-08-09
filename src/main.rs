extern crate backup_rs as app; // app
#[macro_use]
extern crate clap;
extern crate libc;
#[macro_use]
extern crate log;

use app::actions;
use app::logger::term::{self, highlight};
use app::Result;
use clap::ArgMatches;
use libc::EXIT_FAILURE;
use std::path::PathBuf;
use std::process::exit;

fn main() {
    if let Err(_) = term::init() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    let yaml = load_yaml!("cli.yml");
    if let Err(err) = App::new(clap::App::from(yaml).get_matches()).run() {
        error!("{}", err);
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
                highlight(self.path.display())
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
        actions::backup::Backup::new(&self.app.path).execute()?;
        Ok(())
    }
}
