#![allow(deprecated)]
#![allow(unused_imports)]
#![allow(clippy::new_ret_no_self)]

//! This is the CLI implementation for the artid application, it allows the app to run from
//! the command line or to start as a GUI.
//!
//! The core of the application lives on the lib directory. This file and its modules job
//! is to parse the command line arguments and transform them into the proper calls to the
//! core.

use libc::EXIT_FAILURE;
use log::{error, log};
use std::error::Error;
use std::process::exit;

macro_rules! curr_dir {
    () => {
        ::std::env::current_dir().expect("Unable to retrieve current work directory")
    };
}

mod errors;
mod ops;
mod parser;

pub type AppResult<T> = Result<T, crate::errors::Error>;

fn main() {
    if logger::init("info").is_err() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    match parser::parse() {
        Ok(app) => if let Err(error) = app.run() {
            if app.backtrace() {
                error!("{}", error);
                let mut source = error.source();
                while let Some(cause) = source {
                    error!("{}", cause);
                    source = cause.source();
                }
            } else {
                error!("{}", error);
            }

            exit(EXIT_FAILURE);
        },

        Err(err) => {
            error!("{}", err);
            exit(EXIT_FAILURE);
        }
    }
}
