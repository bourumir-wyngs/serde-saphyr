use std::{fmt, io};

/// Error type used by the YAML serializer.
///
/// This type is re-exported as `serde_saphyr::ser::Error` and is returned by
/// the public serialization APIs (for example `serde_saphyr::to_string`).
///
/// It implements `serde::ser::Error`, which allows user `Serialize` impls and
/// Serde derives to report failures via `S::Error::custom(...)`. Such
/// free‑form messages are stored in the `Message` variant.
///
/// Other variants wrap concrete underlying failures that can occur while
/// serializing:
/// - `Format` wraps a `std::fmt::Error` produced when writing to a
///   `fmt::Write` target.
/// - `IO` wraps a `std::io::Error` produced when writing to an `io::Write`
///   target.
/// - `Unexpected` is used internally for invariant violations (e.g., around
///   anchors). It should not normally surface; if it does, please file a bug.
#[derive(Debug)]
pub enum Error {
    /// Free-form error.
    Message { msg: String },
    /// Wrapper for formatting errors.
    Format { error: fmt::Error },
    /// Wrapper for I/O errors.
    IO { error: io::Error },
    /// This is used with anchors and should normally not surface, please report bug if it does.
    Unexpected { msg: String },
    /// Options used would produce invalid YAML (0 indentation, etc)
    InvalidOptions(String),
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Message {
            msg: msg.to_string(),
        }
    }
}

impl From<fmt::Error> for Error {
    fn from(error: fmt::Error) -> Self {
        Error::Format { error }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO { error }
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Message { msg: message }
    }
}

impl From<&String> for Error {
    fn from(message: &String) -> Self {
        Error::Message {
            msg: message.clone(),
        }
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Error::Message {
            msg: message.to_string(),
        }
    }
}

impl Error {
    #[cold]
    #[inline(never)]
    pub(crate) fn unexpected(message: &str) -> Self {
        Error::Unexpected {
            msg: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message { msg } => f.write_str(msg),
            Error::Format { error } => write!(f, "formatting error: {error}"),
            Error::IO { error } => write!(f, "I/O error: {error}"),
            Error::Unexpected { msg } => write!(f, "unexpected internal error: {msg}"),
            Error::InvalidOptions(msg) => write!(f, "invalid serialization options: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Message { .. } => None,
            Error::Unexpected { .. } => None,
            Error::Format { error } => Some(error),
            Error::IO { error } => Some(error),
            Error::InvalidOptions(_) => None,
        }
    }
}
