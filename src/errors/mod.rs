use failure::{Backtrace, Context, Fail};

use std::fmt::{self, Display};

use logger::highlight;

mod either;

use self::either::Either;

#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub enum AppErrorType {
    NotDir(String),
    PathUnexistant(String),
    AccessFile(String),
    JsonParse(String),
    UpdateFolder(String),
    RestoreFolder(String),
}

impl Display for AppErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AppErrorType::NotDir(ref dir) => {
                write!(f, "Given path {} is not a dir", highlight(dir))
            }

            AppErrorType::PathUnexistant(ref path) => {
                write!(f, "Given path {} does not exist", highlight(path))
            }

            AppErrorType::AccessFile(ref file) => {
                write!(f, "Given path {} does not exist", highlight(file))
            }

            AppErrorType::JsonParse(ref file) => write!(f, "Unable to parse {}", highlight(file)),

            AppErrorType::UpdateFolder(ref folder) => {
                write!(f, "Unable to sync {}", highlight(folder))
            }

            AppErrorType::RestoreFolder(ref folder) => {
                write!(f, "Unable to sync {}", highlight(folder))
            }
        }
    }
}

#[derive(Debug)]
pub struct AppError {
    inner: Either<Context<AppErrorType>, Context<&'static str>>,
}

impl AppError {
    pub fn kind(&self) -> Option<&AppErrorType> {
        match (*self).inner {
            Either::Enum(ref enumerate) => Some(enumerate.get_context()),
            Either::Str(_) => None,
        }
    }
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

impl From<AppErrorType> for AppError {
    fn from(kind: AppErrorType) -> AppError {
        AppError {
            inner: Either::Enum(Context::new(kind)),
        }
    }
}

impl From<Context<AppErrorType>> for AppError {
    fn from(inner: Context<AppErrorType>) -> AppError {
        AppError {
            inner: Either::Enum(inner),
        }
    }
}

impl From<Context<&'static str>> for AppError {
    fn from(inner: Context<&'static str>) -> AppError {
        AppError {
            inner: Either::Str(inner),
        }
    }
}
