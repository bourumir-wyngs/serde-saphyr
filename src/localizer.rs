//! Localization / wording customization.
//!
//! The [`Localizer`] trait is the central hook for customizing *crate-authored wording*.
//! It is intentionally designed to be low-boilerplate:
//!
//! - Every method has a reasonable English default.
//! - You can override only the pieces you care about, while inheriting all other defaults.
//!
//! This crate may also show *external* message text coming from dependencies (for example
//! `saphyr-parser` scan errors, or validator messages). Where such texts are used, the
//! rendering pipeline should provide a best-effort opportunity to override them via
//! [`Localizer::override_external_message`].
//!
//! ## Example: override a single phrase
//!
//! ```rust
//! use serde_saphyr::{Error, Location};
//! use serde_saphyr::localizer::{Localizer, DEFAULT_ENGLISH_LOCALIZER};
//! use std::borrow::Cow;
//!
//! /// A wrapper that overrides only location suffix wording, delegating everything else.
//! struct Pirate<'a> {
//!     base: &'a dyn Localizer,
//! }
//!
//! impl Localizer for Pirate<'_> {
//!     fn attach_location<'b>(&self, base: Cow<'b, str>, loc: Location) -> Cow<'b, str> {
//!         if loc == Location::UNKNOWN {
//!             return base;
//!         }
//!         // Note: you can also delegate to `self.base.attach_location(...)` if you want.
//!         Cow::Owned(format!(
//!             "{base}. Bug lurks on line {}, then {} runes in",
//!             loc.line(),
//!             loc.column()
//!         ))
//!     }
//! }
//!
//! // This snippet shows the customization building blocks; the crate's rendering APIs
//! // obtain a `Localizer` via the `MessageFormatter`.
//! # let _ = (Error::InvalidUtf8Input, &DEFAULT_ENGLISH_LOCALIZER);
//! ```

use crate::Location;
use std::borrow::Cow;

/// Where an “external” message comes from.
///
/// External messages are those primarily produced by dependencies (parser / validators).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalMessageSource {
    /// Text produced by `saphyr-parser` (e.g. scanning errors).
    SaphyrParser,
    /// Text produced by `garde` validation rules.
    Garde,
    /// Text produced by `validator` validation rules.
    Validator,
}

/// A best-effort description of an external message.
///
/// The crate should pass as much stable metadata as it has (e.g. `code` and `params` for
/// `validator`) so the localizer can override *specific* messages without string matching.
#[derive(Debug, Clone)]
pub struct ExternalMessage<'a> {
    pub source: ExternalMessageSource,
    /// The original text as provided by the external library.
    pub original: &'a str,
    /// Stable-ish identifier when available (e.g. validator error code).
    pub code: Option<&'a str>,
    /// Optional structured parameters when available.
    pub params: &'a [(String, String)],
}

/// All crate-authored wording customization points.
///
/// Implementors should typically override *only a few* methods.
/// Everything else should default to English (via the default method bodies).
pub trait Localizer {
    // ---------------- Common tiny building blocks ----------------

    /// Attach a location suffix to `base`.
    ///
    /// Renderers must use this instead of hard-coding English wording like
    /// `" at line X, column Y"`.
    ///
    /// Default:
    /// - If `loc == Location::UNKNOWN`: returns `base` unchanged.
    /// - Otherwise: returns `"{base} at line {line}, column {column}"`.
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
        if loc == Location::UNKNOWN {
            base
        } else {
            Cow::Owned(format!("{base} at line {}, column {}", loc.line, loc.column))
        }
    }

    /// Label used when a path has no leaf.
    ///
    /// Default empty
    fn root_path_label(&self) -> Cow<'static, str> {
        Cow::Borrowed("")
    }

    /// Suffix for alias-related errors when a distinct defined-location is available.
    ///
    /// Default wording matches the crate's historical English output:
    /// `" (defined at line X, column Y)"`.
    ///
    /// Default: `format!(" (defined at line {line}, column {column})", ...)`.
    fn alias_defined_at(&self, defined: Location) -> String {
        format!(" (defined at line {}, column {})", defined.line, defined.column)
    }

    // ---------------- Validation (plain text) glue ----------------

    /// Render one validation issue line.
    ///
    /// The crate provides `resolved_path`, `entry` and the chosen `loc`.
    ///
    /// Default:
    /// - Base text: `"validation error at {resolved_path}: {entry}"`.
    /// - If `loc` is `Some` and not `Location::UNKNOWN`, appends a location suffix via
    ///   [`Localizer::attach_location`].
    fn validation_issue_line(
        &self,
        resolved_path: &str,
        entry: &str,
        loc: Option<Location>,
    ) -> String {
        let base = format!("validation error at {resolved_path}: {entry}");
        match loc {
            Some(l) if l != Location::UNKNOWN => self.attach_location(Cow::Owned(base), l).into_owned(),
            _ => base,
        }
    }

    /// Join multiple validation issues into one message.
    ///
    /// Default: joins `lines` with a single newline (`"\n"`).
    fn join_validation_issues(&self, lines: &[String]) -> String {
        lines.join("\n")
    }

    // ---------------- Validation snippets / diagnostic labels ----------------

    /// Label used for a snippet window when the location is known and considered the
    /// “definition” site.
    ///
    /// Default: `"(defined)"`.
    fn defined(&self) -> Cow<'static, str> {
        Cow::Borrowed("(defined)")
    }

    /// Label used for a snippet window when we only have a “defined here” location.
    ///
    /// Default: `"(defined here)"`.
    fn defined_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("(defined here)")
    }

    /// Label used for the primary snippet window when an aliased/anchored value is used
    /// at a different location than where it was defined.
    ///
    /// Default: `"the value is used here"`.
    fn value_used_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("the value is used here")
    }

    /// Label used for the secondary snippet window that points at the anchor definition.
    ///
    /// Default: `"defined here"`.
    fn defined_window(&self) -> Cow<'static, str> {
        Cow::Borrowed("defined here")
    }

    /// Compose the base validation message used in snippet rendering.
    ///
    /// Default: `"validation error: {entry} for `{resolved_path}`"`.
    fn validation_base_message(&self, entry: &str, resolved_path: &str) -> String {
        format!("validation error: {entry} for `{resolved_path}`")
    }

    /// Compose the “invalid here” prefix for the primary snippet message.
    ///
    /// Default: `"invalid here, {base}"`.
    fn invalid_here(&self, base: &str) -> String {
        format!("invalid here, {base}")
    }

    /// Intro line printed between the primary and secondary snippet windows for
    /// anchor/alias (“indirect value”) cases.
    ///
    /// Default:
    /// `"  | This value comes indirectly from the anchor at line {line} column {column}:"`.
    fn value_comes_from_the_anchor(&self, def: Location) -> String {
        format!(
            "  | This value comes indirectly from the anchor at line {} column {}:",
            def.line, def.column
        )
    }

    // ---------------- External overrides ----------------

    /// Optional hook to override the location prefix used for external messages.
    ///
    /// Default:
    /// - If `loc == Location::UNKNOWN`: returns an empty string.
    /// - Otherwise: returns `"line {line} column {column}"`.
    fn snippet_location_prefix(&self, loc: Location) -> String {
        if loc == Location::UNKNOWN {
            String::new()
        } else {
            format!("line {} column {}", loc.line(), loc.column())
        }
    }

    /// Best-effort hook to override/translate dependency-provided message text.
    ///
    /// Default: returns `None` (keep the external message as-is).
    fn override_external_message<'a>(&self, _msg: ExternalMessage<'a>) -> Option<Cow<'a, str>> {
        None
    }
}

/// Default English localizer used by the crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultEnglishLocalizer;

impl Localizer for DefaultEnglishLocalizer {}

/// A single shared instance of the default English localizer.
///
/// This avoids repeated instantiation and provides a convenient reference for wrappers.
pub static DEFAULT_ENGLISH_LOCALIZER: DefaultEnglishLocalizer = DefaultEnglishLocalizer;
