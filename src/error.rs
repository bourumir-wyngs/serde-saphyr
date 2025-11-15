//! Defines error and its location
use std::fmt;

use serde::de::{self};
use saphyr_parser::{ScanError, Span};
use crate::budget::BudgetBreach;
use crate::de::Events;

/// Row/column location within the source YAML document (1-indexed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    /// 1-indexed row number in the input stream.
    pub (crate) row: u32,
    /// 1-indexed column number in the input stream.
    pub (crate) column: u32,
}

impl Location {
    /// serde_yaml-compatible line information
    pub fn line(&self) -> u64 { self.row as u64 }

    /// serde_yaml-compatible column information
    pub fn column(&self) -> u64 { self.column as u64}
}

impl Location {
    /// Sentinel value meaning "location unknown".
    ///
    /// Used when a precise position is not yet available at error creation time.
    pub const UNKNOWN: Self = Self { row: 0, column: 0 };

    /// Create a new location record.
    ///
    /// Arguments:
    /// - `row`: 1-indexed row.
    /// - `column`: 1-indexed column.
    ///
    /// Returns:
    /// - `Location` with the provided coordinates.
    ///
    /// Called by:
    /// - Parser/scan adapters that convert upstream spans to `Location`.
    pub(crate) const fn new(row: usize, column: usize) -> Self {
        // 4 Gb is larger than any YAML document I can imagine, and also this is
        // error reporting only.
        Self { row: row as u32, column: column as u32 }
    }
}

/// Convert a `saphyr_parser::Span` to a 1-indexed `Location`.
///
/// Called by:
/// - The live events adapter for each raw parser event.
pub(crate) fn location_from_span(span: &Span) -> Location {
    let start = &span.start;
    Location::new(start.line(), start.col() + 1)
}

/// Error type compatible with `serde::de::Error`.
#[derive(Debug)]
pub enum Error {
    /// Free-form error with optional source location.
    Message {
        msg: String,
        location: Location,
    },
    /// Unexpected end of input.
    Eof {
        location: Location,
    },
    /// Structural/type mismatch — something else than the expected token/value was seen.
    Unexpected {
        expected: &'static str,
        location: Location,
    },
    ContainerEndMismatch {
        location: Location,
    },
    /// Alias references a non-existent anchor id.
    UnknownAnchor {
        id: usize,
        location: Location,
    },
    /// Error when parsing robotic and other extensions beyond standard YAML.
    /// (error in extension hook).
    HookError {
        msg: String,
        location: Location,
    },
    /// A YAML budget limit was exceeded.
    Budget {
        breach: BudgetBreach,
        location: Location,
    },
    /// Unexpected I/O error. This may happen only when deserializing from a reader.
    IOError {
        cause: std::io::Error
    }
}

impl Error {
    /// Construct a `Message` error with no known location.
    ///
    /// Arguments:
    /// - `s`: human-readable message.
    ///
    /// Returns:
    /// - `Error::Message` pointing at [`Location::UNKNOWN`].
    ///
    /// Called by:
    /// - Scalar parsers and helpers throughout this module.
    pub(crate) fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message {
            msg: s.into(),
            location: Location::UNKNOWN
        }
    }

    /// Convenience for an `Unexpected` error pre-filled with a human phrase.
    ///
    /// Arguments:
    /// - `what`: short description like "sequence start".
    ///
    /// Returns:
    /// - `Error::Unexpected` at unknown location.
    ///
    /// Called by:
    /// - Deserializer methods that validate the next event kind.
    pub(crate) fn unexpected(what: &'static str) -> Self {
        Error::Unexpected {
            expected: what,
            location: Location::UNKNOWN
        }
    }

    /// Construct an unexpected end-of-input error with unknown location.
    ///
    /// Used by:
    /// - Lookahead and pull methods when `None` appears prematurely.
    pub(crate) fn eof() -> Self {
        Error::Eof {
            location: Location::UNKNOWN
        }
    }

    /// Construct an `UnknownAnchor` error for the given anchor id (unknown location).
    ///
    /// Called by:
    /// - Alias replay logic in the live event source.
    pub(crate) fn unknown_anchor(id: usize) -> Self {
        Error::UnknownAnchor {
            id,
            location: Location::UNKNOWN
        }
    }

    /// Attach/override a concrete location to this error and return it.
    ///
    /// Arguments:
    /// - `set_location`: location to store in the error.
    ///
    /// Returns:
    /// - The same `Error` with location updated.
    ///
    /// Called by:
    /// - Most error paths once the event position becomes known.
    pub(crate) fn with_location(mut self, set_location: Location) -> Self {
        match &mut self {
            Error::Message { location, .. }
            | Error::Eof { location }
            | Error::Unexpected { location, .. }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::Budget { location, .. } => {
                *location = set_location;
            },
            Error::IOError { .. } => {} // this error does not support location
        }
        self
    }

    /// Attach/override a concrete location to this error and return it.
    ///
    /// Arguments:
    /// - `event`: event providing location
    ///
    /// Returns:
    /// - The same `Error` with location updated.
    ///
    /// Called by:
    /// - Most error paths once the event position becomes known.
    pub(crate) fn with_event_location(self, event: &dyn Events) -> Self {
        self.with_location(event.last_location())
    }

    /// If the error has a known location, return it.
    ///
    /// Returns:
    /// - `Some(Location)` when coordinates are known; `None` otherwise.
    ///
    /// Used by:
    /// - Callers that want to surface precise positions to users.
    pub fn location(&self) -> Option<Location> {
        match self {
            Error::Message { location, .. }
            | Error::Eof { location }
            | Error::Unexpected { location, .. }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::Budget { location, .. } => {
                if location != &Location::UNKNOWN {
                    Some(*location)
                } else {
                    None
                }
            },
            Error:: IOError { cause: _ } => None,
        }
    }

    /// Map a `saphyr_parser::ScanError` into our error type with location.
    ///
    /// Called by:
    /// - The live events adapter when the underlying parser fails.
    pub(crate) fn from_scan_error(err: ScanError) -> Self {
        let mark = err.marker();
        let location = Location::new(mark.line(), mark.col() + 1);
        Error::Message {
            msg: err.info().to_owned(),
            location,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message { msg, location } => fmt_with_location(f, msg, location),
            Error::HookError { msg, location } => fmt_with_location(f, msg, location),
            Error::Eof { location } => fmt_with_location(f, "unexpected end of input", location),
            Error::Unexpected { expected, location } => {
                fmt_with_location(f, &format!("unexpected event: expected {expected}"), location)
            }
            Error::ContainerEndMismatch { location } => {
                fmt_with_location(f, "list or mapping end with no start", location)
            }
            Error::UnknownAnchor { id, location } => {
                fmt_with_location(f, &format!("alias references unknown anchor id {id}"), location)
            }
            Error::Budget { breach, location } => {
                fmt_with_location(f, &format!("YAML budget breached: {breach:?}"), location)
            }
            Error::IOError { cause } => write!(f, "IO error: {}", cause),
        }
    }
}
impl std::error::Error for Error {}
impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::msg(msg.to_string())
    }
}

/// Print a message optionally suffixed with "at line X, column Y".
///
/// Arguments:
/// - `f`: destination formatter.
/// - `msg`: main text.
/// - `location`: position to attach if known.
///
/// Returns:
/// - `fmt::Result` as required by `Display`.
fn fmt_with_location(f: &mut fmt::Formatter<'_>, msg: &str, location: &Location) -> fmt::Result {
    if location != &Location::UNKNOWN {
        write!(
            f,
            "{msg} at line {}, column {}",
            location.row, location.column
        )
    } else {
        write!(f, "{msg}")
    }
}

/// Convert a budget breach report into a user-facing error.
///
/// Arguments:
/// - `breach`: which limit was exceeded (from the streaming budget checker).
///
/// Returns:
/// - `Error::Message` with a formatted description.
///
/// Called by:
/// - The live events layer when enforcing budgets during/after parsing.
pub(crate) fn budget_error(breach: BudgetBreach) -> Error {
    Error::Budget { breach, location: Location::UNKNOWN }
}
