use super::super::logger::term::highlight;
use std::fmt;

pub fn not_a_dir(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "{} is not a directory", highlight(path))
}

pub fn path_unexistant(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "{} does not exist", highlight(path))
}

pub fn access(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "{} is not accessible", highlight(path))
}

pub fn json_parse(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "impossible to parse {}", highlight(path))
}

pub fn update(f: &mut fmt::Formatter, path: &String) -> fmt::Result {
    write!(f, "impossible to update {}", highlight(path))
}
