//! Tools for general module error creation

/// Creates the basic boilerplate for the module errors
#[macro_export]
macro_rules! create_error {
    () => {
        /// Represents the errors returned by the module specific functions
        #[derive(Debug)]
        pub struct Error {
            kind: ErrorKind,
            cause: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
        }

        impl Error {
            #[allow(missing_docs)]
            pub fn new(kind: ErrorKind) -> Self {
                Self { kind, cause: None }
            }

            #[allow(missing_docs)]
            pub fn with_cause<E>(kind: ErrorKind, cause: E) -> Self
            where
                E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
            {
                Self {
                    kind,
                    cause: Some(cause.into()),
                }
            }

            #[allow(missing_docs)]
            pub fn kind(&self) -> ErrorKind {
                self.kind
            }
        }

        impl std::fmt::Display for Error {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "[{}] -> {}", self.kind.code(), self.kind.to_string())
            }
        }

        impl std::error::Error for Error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self.cause {
                    Some(ref boxed) => Some(&**boxed),
                    None => None,
                }
            }
        }

        impl From<ErrorKind> for Error {
            fn from(kind: ErrorKind) -> Self {
                Error::new(kind)
            }
        }
    };
}
