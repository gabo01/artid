/// This file contains the implementation of the top-level application error.
///
/// This error is responsible for showing the the general applciation error cause to the
/// user and to contain a backtrace that the user can request to be displayed.
use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

use app::errors::{FileError, OperativeError};

#[derive(Debug)]
pub struct AppError {
    inner: Context<ErrorType>,
}

impl Fail for AppError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

#[derive(Copy, Clone, Debug, Fail, Eq, PartialEq)]
pub enum ErrorType {
    #[fail(display = "Unable to perform the requested operation")]
    Operative,
    #[fail(display = "Unable to operate on the configuration file")]
    Config,
}

impl From<ErrorType> for AppError {
    fn from(kind: ErrorType) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorType>> for AppError {
    fn from(inner: Context<ErrorType>) -> Self {
        Self { inner }
    }
}

impl From<FileError> for AppError {
    fn from(error: FileError) -> Self {
        Self {
            inner: error.context(ErrorType::Config),
        }
    }
}

impl From<OperativeError> for AppError {
    fn from(error: OperativeError) -> Self {
        Self {
            inner: error.context(ErrorType::Operative),
        }
    }
}
