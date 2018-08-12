use env_logger::{Builder, Env, DEFAULT_FILTER_ENV};
use libc;
use log;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use yansi::{Color, Paint};

pub fn init(filter_level: &str) -> Result<(), log::SetLoggerError> {
    if !OutputStream::is_ansi() {
        Paint::disable();
    }
    init_builder(if cfg!(debug_assertions) {
        "trace"
    } else {
        filter_level
    })
}

pub fn highlight<M: Display>(input: M) -> Paint<M> {
    Color::Cyan.paint(input).bold()
}

pub fn pathlight<P: AsRef<Path>>(path: P) -> Paint<String> {
    highlight(path.as_ref().display().to_string())
}

struct OutputStream;

impl OutputStream {
    pub fn is_ansi() -> bool {
        if cfg!(not(target_os = "linux")) {
            false
        } else {
            Self::is_output_term()
        }
    }

    fn is_output_term() -> bool {
        (unsafe { libc::isatty(libc::STDERR_FILENO as i32) } != 0)
    }
}

fn init_builder(filter_level: &str) -> Result<(), log::SetLoggerError> {
    let mut builder = Builder::from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, filter_level));

    builder.format(|buf, record| writeln!(buf, "{}: {}", style_level(&record), record.args()));
    builder.try_init()
}

fn style_level(record: &log::Record<'_>) -> Paint<String> {
    let string = record.level().to_string().to_lowercase();
    match &*string {
        "trace" => Color::White.paint(string).bold(),
        "debug" => Color::Cyan.paint(string).bold(),
        "info" => Color::Green.paint(string).bold(),
        "warn" => Color::Yellow.paint(string).bold(),
        "error" => Color::Red.paint(string).bold(),
        _ => unreachable!(),
    }
}
