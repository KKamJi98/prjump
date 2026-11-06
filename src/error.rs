use std::fmt;

/// Errors the CLI reports to the user.
///
/// The two variants keep the two message prefixes of prjump 0.2.2:
/// `User` errors print `Error: <msg>`, everything else prints
/// `Unexpected error: <msg>`. Both exit with status 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    User(String),
    Unexpected(String),
}

impl Error {
    pub fn user(msg: impl Into<String>) -> Self {
        Error::User(msg.into())
    }

    pub fn unexpected(msg: impl Into<String>) -> Self {
        Error::Unexpected(msg.into())
    }

    pub fn message(&self) -> &str {
        match self {
            Error::User(msg) | Error::Unexpected(msg) => msg,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::User(msg) => write!(f, "Error: {msg}"),
            Error::Unexpected(msg) => write!(f, "Unexpected error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}
