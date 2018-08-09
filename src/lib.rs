extern crate chrono;

use chrono::DateTime;
use chrono::offset::Utc;

pub mod actions;
pub mod logger;

pub struct ConfigFile {
    folders: Vec<Folder>
}

pub struct Folder {
    path: String,
    origin: String,
    description: String,
    modified: Option<DateTime<Utc>> // parses from an RFC3339 valid string
}
