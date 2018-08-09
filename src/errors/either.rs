use failure::{Backtrace, Fail};
use std::fmt;
use std::fmt::{Debug, Display};
use std::marker::{Send, Sync};


#[derive(Debug)]
pub enum Either<T, U>
	where T: Debug + Display + Send + Sync + 'static + Fail,
		  U: Debug + Display + Send + Sync + 'static + Fail
{
	Enum(T),
	Str(U)
}

impl<T, U> Fail for Either<T, U>
    where T: Debug + Display + Send + Sync + 'static + Fail,
          U: Debug + Display + Send + Sync + 'static + Fail
{
	fn cause(&self) -> Option<&Fail> {
		match *self {
			Either::Enum(ref context) => context.cause(),
			Either::Str(ref context) => context.cause()
		}
    }

    fn backtrace(&self) -> Option<&Backtrace> {
		match *self {
			Either::Enum(ref context) => context.backtrace(),
			Either::Str(ref context) => context.backtrace()
		}
    }
}

impl<T, U> Display for Either<T, U>
    where T: Debug + Display + Send + Sync + 'static + Fail,
          U: Debug + Display + Send + Sync + 'static + Fail
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Either::Enum(ref context) => Display::fmt(context, f),
			Either::Str(ref context) => Display::fmt(context, f)
		}
    }
}
