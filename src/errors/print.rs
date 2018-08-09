use std::fmt;
use super::super::logger::term::highlight;

pub fn not_a_dir(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "{} is not a directory", highlight(path))
}

pub fn path_unexistant(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "{} does not exist", highlight(path))
}
