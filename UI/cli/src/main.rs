#![allow(deprecated)]
#![allow(unused_imports)]
#![allow(clippy::new_ret_no_self)]

//! This is the CLI implementation for the artid application, it allows the app to run from
//! the command line or to start as a GUI.
//!
//! The core of the application lives on the lib directory. This file and its modules job
//! is to parse the command line arguments and transform them into the proper calls to the
//! core.

use clap::{crate_authors, crate_description, crate_version, load_yaml, App};
use libc::EXIT_FAILURE;
use log::{error, log};
use std::error::Error;
use std::process::exit;

mod errors;
mod ops;
mod parser;

use crate::parser::Instance;

pub type AppResult<T> = Result<T, crate::errors::Error>;

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

    let instance = match Instance::new(&matches) {
        Ok(instance) => instance,
        Err(err) => {
            error!("{}", err);
            exit(EXIT_FAILURE);
        }
    };

    if let Err(err) = instance.run() {
        if instance.backtrace() {
            error!("{}", err);

            let mut source = err.source();
            while let Some(cause) = source {
                error!("{}", cause);
                source = cause.source();
            }
        } else {
            error!("{}", err);
        }

        exit(EXIT_FAILURE);
    }
}
