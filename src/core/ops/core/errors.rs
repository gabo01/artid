//! Contains the errors returned by the operations performed by artid

use std::fmt::{self, Display};
use std::io;

create_error! {}

impl From<io::Error> for Error {
    fn from(cause: io::Error) -> Self {
        Error::with_cause(ErrorKind::IO, cause)
    }
}

/// Particular type of error that was encountered during execution
#[derive(Copy, Clone, Debug)]
pub enum ErrorKind {
    /// Errors related to input-output operations
    IO,
    /// Encountered when trying to work with an unexistant snapshot
    PointNotExists,
}

impl ErrorKind {
    #[allow(missing_docs)]
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::IO => "Ops:0000",
            ErrorKind::PointNotExists => "Ops:0001",
        }
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorKind::IO => write!(f, "Unable to access the disk during the operation"),
            ErrorKind::PointNotExists => {
                write!(f, "Requested point for the operation is not valid")
            }
        }
    }
}
