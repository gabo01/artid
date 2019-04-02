//! Contains the error returned by specific failure of config related tasks

use std::fmt::{self, Display};
use std::{error, io};

create_error! {}

impl From<io::Error> for Error {
    fn from(cause: io::Error) -> Self {
        Error::with_cause(ErrorKind::File, cause)
    }
}

impl From<toml::de::Error> for Error {
    fn from(cause: toml::de::Error) -> Self {
        Error::with_cause(ErrorKind::InvalidData, cause)
    }
}

/// Particular type of error that was encountered during execution
#[derive(Copy, Clone, Debug)]
pub enum ErrorKind {
    /// Errors found while performing I/O on disk files
    File,
    /// Errors found while trying to parse the content of the files
    InvalidData,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorKind::File => write!(f, "Unable to access the requested file"),
            ErrorKind::InvalidData => write!(f, "Unable to parse the given configuration"),
        }
    }
}

impl ErrorKind {
    #[allow(missing_docs)]
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::File => "Config:0000",
            ErrorKind::InvalidData => "Config:0001",
        }
    }
}
