//! Contains the errors throwed by the config module.
//! 
//! These errors are mostly related with failure in accessing to the configuration file.

use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

use logger::highlight;

pub type PathRepr = String;

/// Underlying cause of failure when trying to load a file
#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub(super) enum LoadErrorType {
    File(PathRepr),
    Parse(PathRepr),
}

impl Display for LoadErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadErrorType::File(ref path) => write!(
                f,
                "Unable to read configuration from disk path {}",
                highlight(path)
            ),

            LoadErrorType::Parse(ref path) => write!(
                f,
                "Configuration format on disk path {} is not valid",
                highlight(path)
            ),
        }
    }
}

/// Represents failure while trying to load the configuration file
#[derive(Debug)]
pub struct LoadError {
    inner: Context<LoadErrorType>,
}

impl Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Fail for LoadError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl From<LoadErrorType> for LoadError {
    fn from(kind: LoadErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<LoadErrorType>> for LoadError {
    fn from(inner: Context<LoadErrorType>) -> Self {
        Self { inner }
    }
}

/// Represents the underlying failure while trying to save the configuration file
#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub(super) enum SaveErrorType {
    File(PathRepr),
}

impl Display for SaveErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SaveErrorType::File(ref path) => write!(
                f,
                "Unable to save configuration into disk path {}",
                highlight(path)
            ),
        }
    }
}

/// Represents failure while trying to save the configuration file
#[derive(Debug)]
pub struct SaveError {
    inner: Context<SaveErrorType>,
}

impl Fail for SaveError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<SaveErrorType> for SaveError {
    fn from(kind: SaveErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<SaveErrorType>> for SaveError {
    fn from(inner: Context<SaveErrorType>) -> Self {
        Self { inner }
    }
}
