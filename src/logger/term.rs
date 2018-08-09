extern crate env_logger;
extern crate log;
extern crate yansi;

use self::env_logger::{Builder, Env, DEFAULT_FILTER_ENV};
use self::yansi::{Color, Paint};
use std::fmt::Display;
use std::io::Write;

pub fn init() -> Result<(), log::SetLoggerError> {
    let mut builder = Builder::from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));

    builder.format(|buf, record| {
        let log_level = record.level().to_string().to_lowercase();
        writeln!(buf, "{}: {}", level(&log_level), record.args())
    });

    builder.try_init()
}

fn level(level: &str) -> Paint<&str> {
    match level {
        "trace" => Color::White.paint(level).bold(),
        "debug" => Color::Cyan.paint(level).bold(),
        "info" => Color::Green.paint(level).bold(),
        "warn" => Color::Yellow.paint(level).bold(),
        "error" => Color::Red.paint(level).bold(),
        _ => unreachable!(),
    }
}

pub fn highlight<T: Display>(input: T) -> Paint<T> {
    Color::Cyan.paint(input).bold()
}
