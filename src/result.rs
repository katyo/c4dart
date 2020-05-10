use std::{
    error::Error as StdError,
    result::Result as StdResult,
    io::Error as IoError,
    fmt::{Display, Formatter, Result as FmtResult},
};

/// Result type
pub type Result<T> = StdResult<T, Error>;

/// Error type
#[derive(Debug)]
pub enum Error {
    Gen(String),
    Io(IoError),
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use Error::*;

        match self {
            Gen(e) => write!(f, "Generic error: {}", e),
            Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Gen(s)
    }
}

impl<'a> From<&'a str> for Error {
    fn from(s: &'a str) -> Self {
        Error::Gen(s.into())
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::Io(e)
    }
}
