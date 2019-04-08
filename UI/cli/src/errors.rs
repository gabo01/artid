//! This file contains the implementation of the top-level application error.
//!
//! This error is responsible for showing the the general applciation error cause to the
//! user.
use std::error;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Box<dyn error::Error + Send + Sync + 'static>>,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, cause: None }
    }

    pub fn with_cause<E>(kind: ErrorKind, cause: E) -> Self
    where
        E: Into<Box<dyn error::Error + Send + Sync + 'static>>,
    {
        Self {
            kind,
            cause: Some(cause.into()),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.cause {
            Some(ref error) => {
                let code = find_code(&**error);
                match code {
                    Some(code) => write!(f, "{}. Error code [{}]", self.kind.message(), code),
                    None => write!(f, "{}", self.kind.message()),
                }
            }
            _ => write!(f, "{}", self.kind.message()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.cause {
            Some(ref cause) => Some(&**cause),
            None => None,
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error::new(kind)
    }
}

impl From<artid::config::Error> for Error {
    fn from(error: artid::config::Error) -> Self {
        Error::with_cause(ErrorKind::InvalidConfig, error)
    }
}

impl From<artid::ops::Error> for Error {
    fn from(error: artid::ops::Error) -> Self {
        Error::with_cause(ErrorKind::DiskAccess, error)
    }
}

#[derive(Clone, Debug)]
pub enum ErrorKind {
    InvalidInput { arg: String, value: String },
    InvalidConfig,
    DiskAccess,
}

impl ErrorKind {
    pub fn message(&self) -> String {
        match self {
            ErrorKind::InvalidInput { ref arg, ref value } => {
                format!("Invalid value {} given to the argument {}", value, arg)
            }
            ErrorKind::InvalidConfig => "Unable to parse the configuration file".to_string(),
            ErrorKind::DiskAccess => "Unable to access to the filesystem".to_string(),
        }
    }
}

fn find_code<'a>(error: &'a (dyn error::Error + Send + Sync + 'static)) -> Option<&'a str> {
    if let Some(err) = error.downcast_ref::<artid::config::Error>() {
        return Some(err.kind().code());
    }

    if let Some(err) = error.downcast_ref::<artid::ops::Error>() {
        return Some(err.kind().code());
    }

    None
}
