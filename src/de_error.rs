//! Defines error and its location
use std::fmt;

#[cfg(feature = "garde")]
use std::collections::HashMap;
use annotate_snippets::Level;
use crate::budget::BudgetBreach;
use crate::parse_scalars::{
    parse_int_signed, parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish,
};
use crate::tags::SfTag;
use crate::Location;
use saphyr_parser::{ScalarStyle, ScanError};
use serde::de::{self};

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
        cause: std::io::Error,
    },
    /// The value is targeted to the string field but can be interpreted as a number or boolean.
    /// This error can only happens if no_schema set true.
    QuotingRequired {
        value: String, // sanitized (checked) value that must be quoted
        location: Location,
    },

    /// Wrap an error with the full input text, enabling rustc-like snippet rendering.
    WithSnippet {
        /// Pre-rendered snippet output (cropped) for display.
        ///
        /// Note: this intentionally does NOT store the full input text, to avoid
        /// retaining large YAML inputs inside errors.
        text: String,
        crop_radius: usize,
        error: Box<Error>,
    },

    /// Garde validation failure.
    #[cfg(feature = "garde")]
    ValidationError {
        report: garde::Report,
        referenced: HashMap<garde::error::Path, Location>,
        defined: HashMap<garde::error::Path, Location>,
    },

    /// Garde validation failures (multiple, when working with multiple documents)
    #[cfg(feature = "garde")]
    ValidationErrors {
        errors: Vec<Error>,
    },
}

impl Error {
    pub(crate) fn with_snippet(self, text: &str, crop_radius: usize) -> Self {
        // Avoid nesting snippet wrappers: keep the innermost error and rebuild the
        // wrapper with freshly rendered/cropped snippet output.
        let inner = match self {
            Error::WithSnippet { error, .. } => *error,
            other => other,
        };

        let rendered = render_error_with_snippets(&inner, text, crop_radius);

        Error::WithSnippet {
            text: rendered,
            crop_radius,
            error: Box::new(inner),
        }
    }
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
            location: Location::UNKNOWN,
        }
    }

    /// Construct a `QuotingRequired` error with no known location.
    /// Called by:
    /// - Deserializer, when deserializing into string if no_schema set to true.
    pub(crate) fn quoting_required(value: &str) -> Self {
        // Ensure the value really is like number or boolean (do not reflect back content
        // that may be used for attack)
        let location = Location::UNKNOWN;
        let value = if parse_yaml12_float::<f64>(value, location, SfTag::None, false).is_ok()
            || parse_int_signed::<i128>(value, "i128", location, false).is_ok()
            || parse_yaml11_bool(value).is_ok()
            || scalar_is_nullish(value, &ScalarStyle::Plain)
        {
            value.to_string()
        } else {
            String::new()
        };
        Error::QuotingRequired { value, location }
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
            location: Location::UNKNOWN,
        }
    }

    /// Construct an unexpected end-of-input error with unknown location.
    ///
    /// Used by:
    /// - Lookahead and pull methods when `None` appears prematurely.
    pub(crate) fn eof() -> Self {
        Error::Eof {
            location: Location::UNKNOWN,
        }
    }

    /// Construct an `UnknownAnchor` error for the given anchor id (unknown location).
    ///
    /// Called by:
    /// - Alias replay logic in the live event source.
    pub(crate) fn unknown_anchor(id: usize) -> Self {
        Error::UnknownAnchor {
            id,
            location: Location::UNKNOWN,
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
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. } => {
                *location = set_location;
            }
            Error::IOError { .. } => {} // this error does not support location
            Error::WithSnippet { error, .. } => {
                let inner = *std::mem::replace(error, Box::new(Error::eof()));
                *error = Box::new(inner.with_location(set_location));
            }
            #[cfg(feature = "garde")]
            Error::ValidationError { .. } => {
                // Validation errors carry their own per-path locations.
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { .. } => {
                // Aggregate validation errors carry their own per-entry locations.
            }
        }
        self
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
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. } => {
                if location != &Location::UNKNOWN {
                    Some(*location)
                } else {
                    None
                }
            }
            Error::IOError { cause: _ } => None,
            Error::WithSnippet { error, .. } => error.location(),
            #[cfg(feature = "garde")]
            Error::ValidationError {
                referenced, defined, ..
            } => referenced
                .values()
                .copied()
                .find(|l| *l != Location::UNKNOWN)
                .or_else(|| {
                    defined
                        .values()
                        .copied()
                        .find(|l| *l != Location::UNKNOWN)
                }),
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.iter().find_map(|e| e.location()),
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
            Error::WithSnippet { text, crop_radius, error } => {
                if *crop_radius == 0 {
                    // Treat as "snippet disabled".
                    return write!(f, "{}", error);
                }
                // `text` is already the pre-rendered snippet output.
                write!(f, "{text}")
            }
            Error::Message { msg, location } => fmt_with_location(f, msg, location),
            Error::HookError { msg, location } => fmt_with_location(f, msg, location),
            Error::Eof { location } => fmt_with_location(f, "unexpected end of input", location),
            Error::Unexpected { expected, location } => fmt_with_location(
                f,
                &format!("unexpected event: expected {expected}"),
                location,
            ),
            Error::ContainerEndMismatch { location } => {
                fmt_with_location(f, "list or mapping end with no start", location)
            }
            Error::UnknownAnchor { id, location } => fmt_with_location(
                f,
                &format!("alias references unknown anchor id {id}"),
                location,
            ),
            Error::Budget { breach, location } => {
                fmt_with_location(f, &format!("YAML budget breached: {breach:?}"), location)
            }
            Error::QuotingRequired { value, location } => fmt_with_location(
                f,
                &format!("The string value [{value}] must be quoted"),
                location,
            ),
            Error::IOError { cause } => write!(f, "IO error: {}", cause),

            #[cfg(feature = "garde")]
            Error::ValidationError {
                report,
                referenced,
                defined,
            } => {
                // No input text available here, so we fall back to a location-suffixed
                // message format (snippets are only rendered via `Error::WithSnippet`).
                fmt_validation_error_plain(f, report, referenced, defined)
            }

            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => {
                let mut first = true;
                for err in errors {
                    if !first {
                        writeln!(f)?;
                        writeln!(f)?;
                    }
                    first = false;
                    write!(f, "{err}")?;
                }
                Ok(())
            }
        }
    }
}

fn render_error_with_snippets(inner: &Error, text: &str, crop_radius: usize) -> String {
    // Safety: snippet rendering is best-effort; if anything goes wrong we fall back
    // to the plain error message.
    if crop_radius == 0 {
        return inner.to_string();
    }


    #[cfg(feature = "garde")]
    {
        // Normalize the snippet text to match the coordinate system used by parsing.
        // Our string-based entry points ignore a single leading UTF-8 BOM (`\u{FEFF}`)
        // before parsing, so any `Location { line, column }` we store is relative to
        // the BOM-stripped view. Strip it here as well to keep caret positions aligned.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        if let Error::ValidationError {
            report,
            referenced,
            defined,
        } = inner
        {
            struct ValidationSnippetDisplay<'a> {
                report: &'a garde::Report,
                referenced: &'a HashMap<garde::error::Path, Location>,
                defined: &'a HashMap<garde::error::Path, Location>,
                text: &'a str,
                crop_radius: usize,
            }

            impl fmt::Display for ValidationSnippetDisplay<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt_validation_error_with_snippets(
                        f,
                        self.report,
                        self.referenced,
                        self.defined,
                        self.text,
                        self.crop_radius,
                    )
                }
            }

            return ValidationSnippetDisplay {
                report,
                referenced,
                defined,
                text,
                crop_radius,
            }
            .to_string();
        }

        if let Error::ValidationErrors { errors } = inner {
            let mut out = String::new();
            for (i, err) in errors.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    out.push('\n');
                }

                // If the nested error already contains snippet output, keep it.
                // Otherwise, render it against the shared input text.
                match err {
                    Error::WithSnippet { .. } => out.push_str(&err.to_string()),
                    other => out.push_str(&render_error_with_snippets(other, text, crop_radius)),
                }
            }
            return out;
        }
    }

    let msg = inner.to_string();
    let Some(location) = inner.location() else {
        return msg;
    };

    struct SnippetDisplay<'a> {
        msg: &'a str,
        location: &'a Location,
        text: &'a str,
        crop_radius: usize,
    }

    impl fmt::Display for SnippetDisplay<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            crate::de_snipped::fmt_with_snippet_or_fallback(
                f,
                Level::ERROR,
                self.msg,
                self.location,
                self.text,
                "<input>",
                self.crop_radius,
            )
        }
    }

    SnippetDisplay {
        msg: &msg,
        location: &location,
        text,
        crop_radius,
    }
    .to_string()
}

#[cfg(feature = "garde")]
fn fmt_validation_error_plain(
    f: &mut fmt::Formatter<'_>,
    report: &garde::Report,
    referenced: &HashMap<garde::error::Path, Location>,
    defined: &HashMap<garde::error::Path, Location>,
) -> fmt::Result {
    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;
        let msg = format!("validation error at {path}: {entry}");
        let loc = referenced
            .get(path)
            .or_else(|| defined.get(path))
            .copied()
            .unwrap_or(Location::UNKNOWN);
        fmt_with_location(f, &msg, &loc)?;
    }
    Ok(())
}

#[cfg(feature = "garde")]
fn fmt_validation_error_with_snippets(
    f: &mut fmt::Formatter<'_>,
    report: &garde::Report,
    referenced: &HashMap<garde::error::Path, Location>,
    defined: &HashMap<garde::error::Path, Location>,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let base_msg = format!("validation error: {entry} for `{path}`");

        let ref_loc = referenced.get(path).copied().unwrap_or(Location::UNKNOWN);
        let def_loc = defined.get(path).copied().unwrap_or(Location::UNKNOWN);

        match (ref_loc, def_loc) {
            (Location::UNKNOWN, Location::UNKNOWN) => {
                write!(f, "{base_msg}")?;
            }
            (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                crate::de_snipped::fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &r,
                    text,
                    "(defined)",
                    crop_radius,
                )?;
            }
            (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                crate::de_snipped::fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &d,
                    text,
                    "(defined here)",
                    crop_radius,
                )?;
            }
            (r, d) => {
                crate::de_snipped::fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &format!("invalid here, {base_msg}"),
                    &r,
                    text,
                    "the value is used here",
                    crop_radius,
                )?;
                writeln!(f)?;
                writeln!(
                    f,
                    "  | This value comes indirectly from the anchor at line {} column {}:",
                    d.line,
                    d.column
                )?;
                crate::de_snipped::fmt_snippet_window_or_fallback(
                    f,
                    &d,
                    text,
                    "defined here",
                    crop_radius,
                )?;
            }
        }
    }
    Ok(())
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
            location.line, location.column
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
    Error::Budget {
        breach,
        location: Location::UNKNOWN,
    }
}
