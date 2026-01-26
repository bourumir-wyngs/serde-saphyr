//! Defines error and its location
use crate::Location;
use crate::budget::BudgetBreach;
use crate::location::Locations;
use crate::parse_scalars::{
    parse_int_signed, parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish,
};
#[cfg(feature = "garde")]
use crate::path_map::path_key_from_garde;
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::{PathKey, PathMap, format_path_with_resolved_leaf};
use crate::tags::SfTag;
use saphyr_parser::{ScalarStyle, ScanError};
use serde::de::{self};
use std::cell::RefCell;
use std::fmt;
#[cfg(feature = "validator")]
use validator::{ValidationErrors, ValidationErrorsKind};

thread_local! {
    // Best-effort fallback location for Serde structural errors that have no inherent span,
    // such as `missing_field`. This is set by the deserializer when entering a container.
    static MISSING_FIELD_FALLBACK: RefCell<Option<Location>> = const { RefCell::new(None) };
}

pub(crate) struct MissingFieldLocationGuard {
    prev: Option<Location>,
}

impl MissingFieldLocationGuard {
    pub(crate) fn new(location: Location) -> Self {
        let prev = MISSING_FIELD_FALLBACK.with(|c| c.replace(Some(location)));
        Self { prev }
    }
}

impl Drop for MissingFieldLocationGuard {
    fn drop(&mut self) {
        MISSING_FIELD_FALLBACK.with(|c| {
            c.replace(self.prev.take());
        });
    }
}

/// Error type compatible with `serde::de::Error`.
#[non_exhaustive]
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
    /// Error related to an alias, with both reference (use-site) and defined (anchor) locations.
    ///
    /// This variant allows reporting both where an alias is used and where the anchor is defined,
    /// which is useful for errors that occur when deserializing aliased values.
    AliasError {
        msg: String,
        locations: Locations,
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
    #[cold]
    #[inline(never)]
    pub(crate) fn with_snippet(self, text: &str, crop_radius: usize) -> Self {
        // Avoid nesting snippet wrappers: keep the innermost error and rebuild the
        // wrapper with freshly rendered/cropped snippet output.
        let inner = match self {
            Error::WithSnippet { error, .. } => *error,
            other => other,
        };

        let rendered = crate::de_snipped::render_error_with_snippets(&inner, text, crop_radius);

        Error::WithSnippet {
            text: rendered,
            crop_radius,
            error: Box::new(inner),
        }
    }

    /// Attach a snippet from a partial YAML fragment (e.g., from `RingReader`).
    ///
    /// This is similar to `with_snippet`, but the `text` is a fragment that starts
    /// at `start_line` (1-based) rather than at line 1. The renderer will adjust
    /// line numbers accordingly.
    #[cold]
    #[inline(never)]
    pub(crate) fn with_snippet_offset(
        self,
        text: &str,
        start_line: usize,
        crop_radius: usize,
    ) -> Self {
        let inner = match self {
            Error::WithSnippet { error, .. } => *error,
            other => other,
        };

        let rendered =
            crate::de_snipped::render_error_with_snippets_offset(&inner, text, start_line, crop_radius);

        Error::WithSnippet {
            text: rendered,
            crop_radius,
            error: Box::new(inner),
        }
    }

    /// Provide "no snippet" version for cases when snippet rendering is not  desired.
    pub fn without_snippet(&self) -> &Self {
        match self {
            Error::WithSnippet { error, .. } => error,
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
    #[cold]
    #[inline(never)]
    pub(crate) fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message {
            msg: s.into(),
            location: Location::UNKNOWN,
        }
    }

    /// Construct a `QuotingRequired` error with no known location.
    /// Called by:
    /// - Deserializer, when deserializing into string if no_schema set to true.
    #[cold]
    #[inline(never)]
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
    #[cold]
    #[inline(never)]
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
    #[cold]
    #[inline(never)]
    pub(crate) fn eof() -> Self {
        Error::Eof {
            location: Location::UNKNOWN,
        }
    }

    /// Construct an `UnknownAnchor` error for the given anchor id (unknown location).
    ///
    /// Called by:
    /// - Alias replay logic in the live event source.
    #[cold]
    #[inline(never)]
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
    #[cold]
    #[inline(never)]
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
            Error::AliasError { .. } => {
                // AliasError carries its own Locations; don't override with a single location.
            }
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
            Error::AliasError { locations, .. } => Locations::primary_location(*locations),
            Error::WithSnippet { error, .. } => error.location(),
            #[cfg(feature = "garde")]
            Error::ValidationError { locations, .. } => locations
                .map
                .values()
                .copied()
                .find_map(Locations::primary_location),
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.iter().find_map(|e| e.location()),
            #[cfg(feature = "validator")]
            Error::ValidatorError { locations, .. } => locations
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
    ///   identical locations.
    /// - For validation errors (when the `garde` / `validator` feature is enabled), this returns
    ///   the `(reference_location, defined_location)` pair for the *first* validation entry.
    ///
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
            | Error::Budget { location, .. } => Locations::same(location),
            Error::IOError { .. } => None,
            Error::AliasError { locations, .. } => Some(*locations),
            Error::WithSnippet { error, .. } => error.locations(),
            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                report.iter().next().and_then(|(path, _)| {
                    let key = path_key_from_garde(path);
                    search_locations_with_ancestor_fallback(locations, &key)
                })
            }
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
    #[cold]
    #[inline(never)]
    pub(crate) fn from_scan_error(err: ScanError) -> Self {
        let mark = err.marker();
        let location =
            Location::new(mark.line(), mark.col() + 1).with_span(crate::location::Span {
                offset: mark.index(),
                len: 1,
            });
        Error::Message {
            msg: err.info().to_owned(),
            location,
        }
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn search_locations_with_ancestor_fallback(
    locations: &PathMap,
    path: &PathKey,
) -> Option<Locations> {
    if let Some((locs, _)) = locations.search(path) {
        return Some(locs);
    }

    let mut p = path.parent();
    while let Some(cur) = p {
        if let Some((locs, _)) = locations.search(&cur) {
            return Some(locs);
        }
        p = cur.parent();
    }

    None
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
            Error::AliasError { msg, locations } => {
                fmt_alias_error_plain(f, msg, locations)
            }

            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
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
            Error::ValidatorError { errors, locations } => {
                fmt_validator_error_plain(f, errors, locations)
            }

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

        let (locs, resolved_leaf) = locations
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

fn maybe_attach_fallback_location(mut err: Error) -> Error {
    let loc = MISSING_FIELD_FALLBACK.with(|c| *c.borrow());
    if let Some(loc) = loc
        && loc != Location::UNKNOWN
    {
        err = err.with_location(loc);
    }
    err
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        // Keep custom errors locationless by default; the deserializer should attach an explicit
        // location when it can. For Serde-generated errors, we override the relevant hooks below
        // and attach a best-effort fallback location.
        Error::msg(msg.to_string())
    }

    fn invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
        // Mirror serde’s default formatting, but add a best-effort location.
        maybe_attach_fallback_location(Error::msg(format!("invalid type: {unexp}, expected {exp}")))
    }

    fn invalid_value(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
        maybe_attach_fallback_location(Error::msg(format!(
            "invalid value: {unexp}, expected {exp}"
        )))
    }

    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        maybe_attach_fallback_location(Error::msg(format!(
            "unknown variant `{variant}`, expected one of {}",
            expected.join(", ")
        )))
    }

    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        maybe_attach_fallback_location(Error::msg(format!(
            "unknown field `{field}`, expected one of {}",
            expected.join(", ")
        )))
    }

    fn missing_field(field: &'static str) -> Self {
        maybe_attach_fallback_location(Error::msg(format!("missing field `{field}`")))
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
#[cold]
#[inline(never)]
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

/// Format an alias error with both reference (use-site) and defined (anchor) locations.
///
/// This provides a plain-text representation showing where the alias is used and where
/// the anchor is defined, which is useful for errors that occur when deserializing aliased values.
#[cold]
#[inline(never)]
fn fmt_alias_error_plain(
    f: &mut fmt::Formatter<'_>,
    msg: &str,
    locations: &Locations,
) -> fmt::Result {
    let ref_loc = locations.reference_location;
    let def_loc = locations.defined_location;

    match (ref_loc, def_loc) {
        (Location::UNKNOWN, Location::UNKNOWN) => {
            write!(f, "{msg}")
        }
        (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
            // Only reference location known, or both are the same
            write!(f, "{msg} at line {}, column {}", r.line, r.column)
        }
        (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
            // Only defined location known
            write!(f, "{msg} (defined at line {}, column {})", d.line, d.column)
        }
        (r, d) => {
            // Both locations known and different
            write!(
                f,
                "{msg} at line {}, column {} (defined at line {}, column {})",
                r.line, r.column, d.line, d.column
            )
        }
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
#[cold]
#[inline(never)]
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
            cause: std::io::Error::other("x"),
        };
        assert_eq!(err.locations(), None);
    }

    #[test]
    fn alias_error_returns_both_locations() {
        let ref_loc = Location::new(5, 10);
        let def_loc = Location::new(2, 3);
        let err = Error::AliasError {
            msg: "test error".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };

        // location() should return the primary (reference) location
        assert_eq!(err.location(), Some(ref_loc));

        // locations() should return both
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            })
        );
    }

    #[test]
    fn alias_error_display_shows_both_locations() {
        let ref_loc = Location::new(5, 10);
        let def_loc = Location::new(2, 3);
        let err = Error::AliasError {
            msg: "invalid value".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };

        let display = err.to_string();
        assert!(display.contains("invalid value"));
        assert!(display.contains("line 5"));
        assert!(display.contains("column 10"));
        assert!(display.contains("line 2"));
        assert!(display.contains("column 3"));
    }

    #[test]
    fn alias_error_display_with_same_locations() {
        let loc = Location::new(3, 7);
        let err = Error::AliasError {
            msg: "test".to_owned(),
            locations: Locations {
                reference_location: loc,
                defined_location: loc,
            },
        };

        let display = err.to_string();
        // When both locations are the same, should only show one
        assert!(display.contains("line 3"));
        assert!(display.contains("column 7"));
        // Should not contain "defined at" since locations are the same
        assert!(!display.contains("defined at"));
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
