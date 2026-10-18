use std::{error::Error as StdError, fmt, io};

#[derive(Debug)]
pub enum Error {
    Invalid(String),
    Cancelled,
    Operation {
        operation: &'static str,
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl Error {
    pub(crate) fn operation(
        operation: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::Operation {
            operation,
            source: Box::new(source),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Cancelled => formatter.write_str("generation cancelled"),
            Self::Operation { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Operation { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::operation("writing output", source)
    }
}
