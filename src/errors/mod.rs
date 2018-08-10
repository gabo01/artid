use failure::{Backtrace, Context, Fail};

use std::fmt::{self, Display};

mod either;
mod print;

use self::either::Either;

#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub enum AppErrorType {
    NotDir(String),
    PathUnexistant(String),
    Access(String),
    JsonParse(String),
    UpdateFolder(String),
}

impl Display for AppErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AppErrorType::NotDir(ref p) => print::not_a_dir(f, p),
            AppErrorType::PathUnexistant(ref p) => print::path_unexistant(f, p),
            AppErrorType::Access(ref p) => print::access(f, p),
            AppErrorType::JsonParse(ref p) => print::json_parse(f, p),
            AppErrorType::UpdateFolder(ref p) => print::update(f, p),
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
