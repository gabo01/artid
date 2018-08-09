extern crate actix;
extern crate backup_rs; // app
extern crate libc;
#[macro_use]
extern crate log;

use backup_rs::logger::term;
use libc::EXIT_FAILURE;

use std::process::exit;

fn main() {
    if let Err(_) = term::init() {
        println!("Unable to start the logging implementation");
        exit(EXIT_FAILURE);
    }

    debug!("This is a debug message");

    let system = actix::System::new("backup-rs");
    system.run();
}
