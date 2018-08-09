extern crate backup_rs; // app
#[macro_use]
extern crate clap;
extern crate libc;

use backup_rs::logger::term;
use clap::ArgMatches;
use libc::EXIT_FAILURE;
use std::process::exit;

fn main() {
    if let Err(_) = term::init() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    let yaml = load_yaml!("cli.yml");
    App::new(clap::App::from(yaml).get_matches()).run();
}

struct App<'a> {
    _matches: ArgMatches<'a>,
}

impl<'a> App<'a> {
    pub fn new(_matches: ArgMatches<'a>) -> Self {
        App { _matches }
    }

    pub fn run(self) {}
}
