//! Defines error and its location
use crate::Location;
use crate::budget::BudgetBreach;
use crate::location::Locations;
use crate::parse_scalars::{
    parse_int_signed, parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish,
};
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::{PathKey, PathMap, format_path_with_resolved_leaf};
#[cfg(feature = "garde")]
use crate::path_map::path_key_from_garde;
use crate::tags::SfTag;
use annotate_snippets::Level;
use saphyr_parser::{ScalarStyle, ScanError};
use serde::de::{self};
use std::fmt;
#[cfg(feature = "validator")]
use validator::{ValidationErrors, ValidationErrorsKind};

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
    /// This error can only happen if no_schema set true.
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
        locations: PathMap,
    },

    /// Garde validation failures (multiple, if multiple validations fail)
    #[cfg(feature = "garde")]
    ValidationErrors {
        errors: Vec<Error>,
    },

    /// Validator validation failure.
    #[cfg(feature = "validator")]
    ValidatorError {
        errors: ValidationErrors,
        locations: PathMap,
    },

    /// Validator validation failures (multiple, if multiple validations fail)
    #[cfg(feature = "validator")]
    ValidatorErrors {
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

    /// Provide "no snippet" version for cases when snippet rendering is not  desired.
    pub fn without_snippet(&self) -> &Self {
        match self {
            Error::WithSnippet {error, .. } => {
                error
            }
            other => other,
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
                **error = inner.with_location(set_location);
            }
            #[cfg(feature = "garde")]
            Error::ValidationError { .. } => {
                // Validation errors carry their own per-path locations.
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { .. } => {
                // Aggregate validation errors carry their own per-entry locations.
            }
            #[cfg(feature = "validator")]
            Error::ValidatorError { .. } => {
                // Validation errors carry their own per-path locations.
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { .. } => {
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
                locations,
                ..
            } => locations
                .map
                .values()
                .copied()
                .find_map(Locations::primary_location),
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.iter().find_map(|e| e.location()),
            #[cfg(feature = "validator")]
            Error::ValidatorError {
                locations,
                ..
            } => locations
                .map
                .values()
                .copied()
                .find_map(Locations::primary_location),
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => errors.iter().find_map(|e| e.location()),
        }
    }

    /// Return a pair of locations associated with this error.
    ///
    /// - For syntax and other errors that carry a single [`Location`], this returns two
    /// identical locations.
    /// - For validation errors (when the `garde` / `validator` feature is enabled), this returns
    ///   the `(reference_location, defined_location)` pair for the *first* validation entry.
    ///   These two locations may differ when YAML anchors/aliases are involved.
    /// - Returns `None` when no meaningful location information is available.
    pub fn locations(&self) -> Option<Locations> {
        match self {
            Error::Message { location, .. }
            | Error::Eof { location }
            | Error::Unexpected { location, .. }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. } => {
                Locations::same(&location)
            }
            Error::IOError { .. } => None,
            Error::WithSnippet { error, .. } => error.locations(),
            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => report
                .iter()
                .next()
                .and_then(|(path, _)| {
                    let key = path_key_from_garde(path);
                    locations.search(&key).map(|(locs, _)| locs)
                }),
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.first().and_then(Error::locations),
            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => collect_validator_entries(errors)
                .first()
                .and_then(|(path, _)| locations.search(path).map(|(locs, _)| locs)),
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => errors.first().and_then(Error::locations),
        }
    }

    /// Map a `saphyr_parser::ScanError` into our error type with location.
    ///
    /// Called by:
    /// - The live events adapter when the underlying parser fails.
    pub(crate) fn from_scan_error(err: ScanError) -> Self {
        let mark = err.marker();
        let location = Location::new(mark.line(), mark.col() + 1).with_span(crate::location::Span {
            offset: mark.index(),
            len: 1,
        });
        Error::Message {
            msg: err.info().to_owned(),
            location,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WithSnippet {
                text,
                crop_radius,
                error,
            } => {
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
                locations,
            } => {
                // No input text available here, so we fall back to a location-suffixed
                // message format (snippets are only rendered via `Error::WithSnippet`).
                fmt_validation_error_plain(f, report, locations)
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

            #[cfg(feature = "validator")]
            Error::ValidatorError {
                errors,
                locations,
            } => fmt_validator_error_plain(f, errors, locations),

            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => {
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

        if let Error::ValidationError { report, locations } = inner
        {
            struct ValidationSnippetDisplay<'a> {
                report: &'a garde::Report,
                locations: &'a PathMap,
                text: &'a str,
                crop_radius: usize,
            }

            impl fmt::Display for ValidationSnippetDisplay<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt_validation_error_with_snippets(
                        f,
                        self.report,
                        self.locations,
                        self.text,
                        self.crop_radius,
                    )
                }
            }

            return ValidationSnippetDisplay {
                report,
                locations,
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

    #[cfg(feature = "validator")]
    {
        // Normalize the snippet text to match the coordinate system used by parsing.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        if let Error::ValidatorError {
            errors,
            locations,
        } = inner
        {
            struct ValidatorSnippetDisplay<'a> {
                errors: &'a ValidationErrors,
                locations: &'a PathMap,
                text: &'a str,
                crop_radius: usize,
            }

            impl fmt::Display for ValidatorSnippetDisplay<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt_validator_error_with_snippets(
                        f,
                        self.errors,
                        self.locations,
                        self.text,
                        self.crop_radius,
                    )
                }
            }

            return ValidatorSnippetDisplay {
                errors,
                locations,
                text,
                crop_radius,
            }
            .to_string();
        }

        if let Error::ValidatorErrors { errors } = inner {
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
    locations: &PathMap,
) -> fmt::Result {
    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;
        let path_key = path_key_from_garde(path);
        let original_leaf = path_key
            .leaf_string()
            .unwrap_or_else(|| "<root>".to_string());

        let (locs, resolved_leaf) =
            locations
                .search(&path_key)
                .unwrap_or((Locations::UNKNOWN, original_leaf));

        let loc = if locs.reference_location != Location::UNKNOWN {
            locs.reference_location
        } else {
            locs.defined_location
        };

        let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
        let msg = format!("validation error at {resolved_path}: {entry}");
        fmt_with_location(f, &msg, &loc)?;
    }
    Ok(())
}

#[cfg(feature = "garde")]
fn fmt_validation_error_with_snippets(
    f: &mut fmt::Formatter<'_>,
    report: &garde::Report,
    locations: &PathMap,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let path_key = path_key_from_garde(path);
        let original_leaf = path_key
            .leaf_string()
            .unwrap_or_else(|| "<root>".to_string());

        let (locs, resolved_leaf) = locations
            .search(&path_key)
            .unwrap_or((Locations::UNKNOWN, original_leaf.clone()));

        let ref_loc = locs.reference_location;
        let def_loc = locs.defined_location;

        let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
        let base_msg = format!("validation error: {entry} for `{resolved_path}`");

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

#[cfg(feature = "validator")]
fn fmt_validator_error_plain(
    f: &mut fmt::Formatter<'_>,
    errors: &ValidationErrors,
    locations: &PathMap,
) -> fmt::Result {
    let entries = collect_validator_entries(errors);
    let mut first = true;

    for (path, entry) in entries {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let original_leaf = path.leaf_string().unwrap_or_else(|| "<root>".to_string());
        let (locs, resolved_leaf) = locations
            .search(&path)
            .unwrap_or((Locations::UNKNOWN, original_leaf));

        let loc = if locs.reference_location != Location::UNKNOWN {
            locs.reference_location
        } else {
            locs.defined_location
        };

        let resolved_path = format_path_with_resolved_leaf(&path, &resolved_leaf);
        let msg = format!("validation error at {resolved_path}: {entry}");
        fmt_with_location(f, &msg, &loc)?;
    }

    Ok(())
}

#[cfg(feature = "validator")]
fn fmt_validator_error_with_snippets(
    f: &mut fmt::Formatter<'_>,
    errors: &ValidationErrors,
    locations: &PathMap,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    let entries = collect_validator_entries(errors);
    let mut first = true;

    for (path, entry) in entries {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let original_leaf = path.leaf_string().unwrap_or_else(|| "<root>".to_string());
        let (locs, resolved_leaf) = locations
            .search(&path)
            .unwrap_or((Locations::UNKNOWN, original_leaf.clone()));

        let resolved_path = format_path_with_resolved_leaf(&path, &resolved_leaf);
        let base_msg = format!("validation error: {entry} for `{resolved_path}`");

        match (locs.reference_location, locs.defined_location) {
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
                    d.line, d.column
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

#[cfg(feature = "validator")]
fn collect_validator_entries(errors: &ValidationErrors) -> Vec<(PathKey, String)> {
    let mut out = Vec::new();
    let root = PathKey::empty();
    collect_validator_entries_inner(errors, &root, &mut out);
    out
}

#[cfg(feature = "validator")]
fn collect_validator_entries_inner(
    errors: &ValidationErrors,
    path: &PathKey,
    out: &mut Vec<(PathKey, String)>,
) {
    for (field, kind) in errors.errors() {
        let field_path = path.clone().join(field.as_ref());
        match kind {
            ValidationErrorsKind::Field(entries) => {
                for entry in entries {
                    out.push((field_path.clone(), entry.to_string()));
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_validator_entries_inner(inner, &field_path, out);
            }
            ValidationErrorsKind::List(list) => {
                for (idx, inner) in list {
                    let index_path = field_path.clone().join(*idx);
                    collect_validator_entries_inner(inner, &index_path, out);
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_for_basic_error_duplicates_location() {
        let l = Location::new(3, 7);
        let err = Error::Message {
            msg: "x".to_owned(),
            location: l,
        };
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: l,
                defined_location: l,
            })
        );
    }

    #[test]
    fn locations_for_io_error_is_unknown() {
        let err = Error::IOError {
            cause: std::io::Error::new(std::io::ErrorKind::Other, "x"),
        };
        assert_eq!(err.locations(), None);
    }

    #[cfg(feature = "validator")]
    #[test]
    fn locations_for_validator_error_uses_first_entry() {
        use validator::Validate;

        #[derive(Debug, Validate)]
        struct Cfg {
            #[validate(length(min = 2))]
            second_string: String,
        }

        let cfg = Cfg {
            second_string: "x".to_owned(),
        };
        let errors = cfg.validate().expect_err("validation error expected");

        let referenced_loc = Location::new(3, 15);
        let defined_loc = Location::new(2, 18);

        let mut locations = PathMap::new();
        locations.insert(
            PathKey::empty().join("secondString"),
            Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            },
        );

        let err = Error::ValidatorError { errors, locations };
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            })
        );
    }
}
