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
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
        if loc == Location::UNKNOWN {
            base
        } else {
            Cow::Owned(format!("{base} at line {}, column {}", loc.line, loc.column))
        }
    }

    /// Label used when a path has no leaf.
    fn root_path_label(&self) -> Cow<'static, str> {
        Cow::Borrowed("<root>")
    }

    /// Suffix for alias-related errors when a distinct defined-location is available.
    ///
    /// Default wording matches the crate's historical English output:
    /// `" (defined at line X, column Y)"`.
    fn alias_defined_at_suffix(&self, defined: Location) -> String {
        format!(" (defined at line {}, column {})", defined.line, defined.column)
    }

    // ---------------- Validation (plain text) glue ----------------

    /// Render one validation issue line.
    ///
    /// The crate provides `resolved_path`, `entry` and the chosen `loc`.
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
    fn join_validation_issues(&self, lines: &[String]) -> String {
        lines.join("\n")
    }

    // ---------------- Validation snippets / diagnostic labels ----------------

    fn snippet_label_defined(&self) -> Cow<'static, str> {
        Cow::Borrowed("(defined)")
    }

    fn snippet_label_defined_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("(defined here)")
    }

    fn snippet_label_value_used_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("the value is used here")
    }

    fn snippet_label_defined_window(&self) -> Cow<'static, str> {
        Cow::Borrowed("defined here")
    }

    fn validation_snippet_base_message(&self, entry: &str, resolved_path: &str) -> String {
        format!("validation error: {entry} for `{resolved_path}`")
    }

    fn validation_snippet_invalid_here(&self, base: &str) -> String {
        format!("invalid here, {base}")
    }

    fn validation_snippet_indirect_anchor_intro(&self, def: Location) -> String {
        format!(
            "  | This value comes indirectly from the anchor at line {} column {}:",
            def.line, def.column
        )
    }

    // ---------------- External overrides ----------------

    /// Optional hook to override an external message.
    ///
    /// Default implementation returns `None` meaning "keep external wording".
    fn snippet_location_prefix(&self, loc: Location) -> String {
        if loc == Location::UNKNOWN {
            String::new()
        } else {
            format!("line {} column {}", loc.line(), loc.column())
        }
    }

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
