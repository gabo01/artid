use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug, Fail, Eq, PartialEq)]
pub enum BackupErrorType {
    #[fail(display = "Unable to read the directory tree")]
    Scan,
    #[fail(display = "Unable to perform the backup operation")]
    Execute,
}

#[derive(Debug)]
pub struct BackupError {
    inner: Context<BackupErrorType>,
}

impl Fail for BackupError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<BackupErrorType> for BackupError {
    fn from(kind: BackupErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<BackupErrorType>> for BackupError {
    fn from(inner: Context<BackupErrorType>) -> Self {
        Self { inner }
    }
}

#[derive(Copy, Clone, Debug, Fail, Eq, PartialEq)]
pub enum RestoreErrorType {
    #[fail(display = "Unable to read the directory tree")]
    Scan,
    #[fail(display = "Unable to perform the backup operation")]
    Execute,
}

#[derive(Debug)]
pub struct RestoreError {
    inner: Context<RestoreErrorType>,
}

impl Fail for RestoreError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<RestoreErrorType> for RestoreError {
    fn from(kind: RestoreErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<RestoreErrorType>> for RestoreError {
    fn from(inner: Context<RestoreErrorType>) -> Self {
        Self { inner }
    }
}
