//! Contains the errors related to the different operations.

use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

/// Represents the underlying cause of failure while trying to perform a backup.
#[derive(Copy, Clone, Debug, Fail, Eq, PartialEq)]
pub enum OperativeErrorType {
    #[fail(display = "Unable to read the directory tree")]
    Scan,
    #[fail(display = "Unable to execute the backup operation model")]
    Backup,
    #[fail(display = "Unable to execute the restore operation model")]
    Restore,
    #[fail(display = "Unable to find the requested folder")]
    FolderDoesNotExists,
}

/// Represents failure while trying to either build a CopyModel for the backup operation
/// or while trying to execute the model.
#[derive(Debug)]
pub struct OperativeError {
    inner: Context<OperativeErrorType>,
}

impl Fail for OperativeError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for OperativeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<OperativeErrorType> for OperativeError {
    fn from(kind: OperativeErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<OperativeErrorType>> for OperativeError {
    fn from(inner: Context<OperativeErrorType>) -> Self {
        Self { inner }
    }
}
