//! Contains the errors throwed by the config module.
//! 
//! These errors are mostly related with failure in accessing to the configuration file.

use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

use logger::highlight;

pub type PathRepr = String;

/// Underlying cause of failure when trying to load a file
#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub(super) enum FileErrorType {
    Load(PathRepr),
    Parse(PathRepr),
    Save(PathRepr),
}

impl Display for FileErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileErrorType::Load(ref path) => write!(
                f,
                "Unable to read configuration from disk path {}",
                highlight(path)
            ),

            FileErrorType::Parse(ref path) => write!(
                f,
                "Configuration format on disk path {} is not valid",
                highlight(path)
            ),

            FileErrorType::Save(ref path) => write!(
                f,
                "Unable to save configuration into disk path {}",
                highlight(path)
            ),
        }
    }
}

/// Represents failure while trying to load the configuration file
#[derive(Debug)]
pub struct FileError {
    inner: Context<FileErrorType>,
}

impl Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Fail for FileError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl From<FileErrorType> for FileError {
    fn from(kind: FileErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<FileErrorType>> for FileError {
    fn from(inner: Context<FileErrorType>) -> Self {
        Self { inner }
    }
}
