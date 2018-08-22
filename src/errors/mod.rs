use failure::{Backtrace, Context, Fail};
use logger::highlight;
use std::fmt::{self, Display};

mod helpers;
pub use self::helpers::PathRepr;

/// Represents an error that happened in the application. It contains a backtrace, if relevant,
/// telling step by step what went wrong in execution. By default it tries to highlight important
/// details about the errors encountered.
#[derive(Debug)]
pub struct AppError {
    inner: Context<AppErrorType>,
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

/// Internal representation of the type of error that happened in the application. This
/// representation contains the kind of error that happened and the associated data.
#[derive(Copy, Clone, Debug, Fail, Eq, PartialEq)]
pub enum AppErrorType {
    #[fail(display = "Unable to access the given disk path")]
    FileSystem,
    #[fail(display = "Unable to parse the config file")]
    JsonParse,
    #[fail(display = "Unable to update the backup folder")]
    UpdateFolder,
    #[fail(display = "Unable to restore the backup")]
    RestoreFolder,
}

impl From<AppErrorType> for AppError {
    fn from(kind: AppErrorType) -> AppError {
        AppError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<AppErrorType>> for AppError {
    fn from(inner: Context<AppErrorType>) -> AppError {
        AppError { inner }
    }
}

#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub enum FsError {
    NotDir(PathRepr),
    PathUnexistant(PathRepr),
    OpenFile(PathRepr),
    CreateFile(PathRepr),
    ReadFile(PathRepr),
    PathExists(PathRepr),
}

impl Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FsError::NotDir(ref path) => write!(f, "{} is not a valid directory", highlight(path)),

            FsError::PathUnexistant(ref path) => write!(f, "{} does not exist", highlight(path)),

            FsError::OpenFile(ref file) => write!(f, "Could not open {}", highlight(file)),

            FsError::CreateFile(ref file) => write!(f, "Could not create {}", highlight(file)),

            FsError::ReadFile(ref file) => write!(f, "Could not read {}", highlight(file)),

            FsError::PathExists(ref path) => write!(
                f,
                "{} already exists, could not write to it",
                highlight(path)
            ),
        }
    }
}

impl From<FsError> for AppError {
    fn from(err: FsError) -> Self {
        Self {
            inner: Context::new(err).context(AppErrorType::FileSystem),
        }
    }
}

impl From<Context<FsError>> for AppError {
    fn from(context: Context<FsError>) -> Self {
        Self {
            inner: context.context(AppErrorType::FileSystem),
        }
    }
}

#[derive(Clone, Debug, Fail, Eq, PartialEq)]
pub enum ParseError {
    JsonParse(PathRepr),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::JsonParse(ref file) => write!(f, "Could not parse {}", highlight(file)),
        }
    }
}

impl From<ParseError> for AppError {
    fn from(err: ParseError) -> Self {
        Self {
            inner: Context::new(err).context(AppErrorType::FileSystem),
        }
    }
}

impl From<Context<ParseError>> for AppError {
    fn from(context: Context<ParseError>) -> Self {
        Self {
            inner: context.context(AppErrorType::JsonParse),
        }
    }
}
