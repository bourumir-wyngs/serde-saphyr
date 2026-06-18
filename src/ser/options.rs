//! Serializer options for YAML emission.
//!
//! Controls indentation and optional anchor name generation for the serializer.
//!
//! Example: use 4-space indentation and a custom anchor naming scheme.
//!
//! ```rust
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Item { a: i32, b: bool }
//!
//! let mut buf = String::new();
//! let opts = serde_saphyr::ser_options! {
//!     indent_step: 4,
//!     anchor_generator: Some(|id| format!("id{}/", id)),
//! };
//! serde_saphyr::to_fmt_writer_with_options(&mut buf, &Item { a: 1, b: true }, opts).unwrap();
//! assert!(buf.contains("a: 1"));
//! ```

use crate::ser_error::Error;

/// Placement style for comments emitted by [`crate::Commented`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentPosition {
    /// Emit the comment inline on the right side from the item it describes (default)
    Inline,
    /// Emit the comment above the item it describes
    Above,
}

/// Scalar spelling used when serializing null values.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NullPolicy {
    /// Emit null values as `null` (default).
    NullNull,
    /// Emit null values as `~`.
    NullTilde,
    /// Emit null values as empty scalars where YAML allows them.
    ///
    /// In flow-style/JSON-like collections such as `[null]` and `{key: null}`,
    /// empty values are not valid, so the serializer falls back to [`NullPolicy::NullNull`].
    NullEmpty,
}

/// Serializer options for YAML emission.
///
/// This struct controls various aspects of YAML serialization, such as indentation,
/// anchor generation, and scalar styles.
///
/// Construct `SerializerOptions` using the [`ser_options!`](crate::ser_options!)
/// macro to ensure compatibility with future updates.
///
/// ```rust
/// use serde::Serialize;
/// use serde_saphyr::{ser_options, to_string_with_options};
///
/// #[derive(Serialize)]
/// struct Config {
///     name: String,
///     values: Vec<i32>,
/// }
///
/// let config = Config {
///     name: "test".to_string(),
///     values: vec![1, 2, 3],
/// };
///
/// // Use 4-space indentation and quote all strings
/// let options = ser_options! {
///     indent_step: 4,
///     quote_all: true,
/// };
///
/// let yaml = to_string_with_options(&config, options).unwrap();
/// ```
#[derive(Clone, Copy)]
pub struct SerializerOptions {
    /// If true, empty maps are emitted as braces {} and empty lists as [] (this is the default).
    /// This form is valid YAML, distinguishes empty collections from null, and may be easier
    /// for a human to read.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub empty_as_braces: bool,
    /// Number of spaces to indent per nesting level when emitting block-style collections (2 by default).
    /// A value of 0 is invalid and will produce an error when serializing, because
    /// no indentation would otherwise produce invalid YAML.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub indent_step: usize,
    /// When enabled, emit list items with a more compact indentation style. On by default.
    /// ```yaml
    ///       containers:
    ///       - env:
    ///         - name: METHOD
    ///           value: WATCH
    /// ```
    ///
    /// Compared to a more expanded indentation style:
    ///
    /// ```yaml
    ///       containers:
    ///         - env:
    ///             - name: METHOD
    ///               value: WATCH
    /// ```
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub compact_list_indent: bool,
    /// Optional custom anchor-name generator.
    ///
    /// Receives a monotonically increasing `usize` id (starting at 1) and returns the
    /// anchor name to emit. If `None`, the built-in generator yields names like `a1`, `a2`, ...
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub anchor_generator: Option<fn(usize) -> String>,
    /// Threshold for block-string wrappers ([crate::LitStr]/[crate::FoldStr] and owned variants
    /// [crate::LitString]/[crate::FoldString]).
    ///
    /// If the string contains a newline, block style is always used. Otherwise, when the
    /// string is single-line and its length is strictly less than this threshold, the
    /// serializer emits a normal YAML scalar (no block style). Longer strings use block
    /// styles `|` or `>` depending on the wrapper. See the type docs for
    /// [crate::LitStr], [crate::FoldStr], [crate::LitString] and [crate::FoldString] for
    /// examples.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub min_fold_chars: usize,
    /// Maximum width (in characters) for lines in folded block scalars (`>`).
    ///
    /// Lines are wrapped **only** at whitespace so that each emitted line is at most
    /// this many characters long (excluding indentation). If no whitespace is present
    /// within the limit (e.g., a single long token), the line is emitted unwrapped
    /// to preserve round-trip correctness: YAML folded scalars typically fold inserted
    /// newlines back as spaces when parsing. Default: 80.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub folded_wrap_chars: usize,
    /// When enabled, serialize simple enums that become a single scalar (unit variants)
    /// using YAML tags, e.g. `!!Enum Variant` instead of a plain scalar `Variant`.
    /// Deserializer does not need this setting as both cases will be understood. Off by default.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub tagged_enums: bool,

    /// When enabled, strings containing more than `folded_wrap_chars` (80 by default) are written
    /// in folded block scalar style (`>`), and strings containing newlines are written in
    /// literal block scalar style (`|`), selecting chomping based on the number of trailing empty lines.
    /// On by default.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub prefer_block_scalars: bool,

    /// When enabled, quote all string scalars. Uses single quotes by default,
    /// but switches to double quotes when the string contains escape sequences
    /// (control characters like `\n`, `\t`, `\r`, backslash) or single quotes.
    /// Disables block scalar styles (`|` and `>`) for quoted strings when active.
    /// Off by default.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub quote_all: bool,

    /// Controls where [`crate::Commented`] comments are emitted in block style.
    ///
    /// [`CommentPosition::Inline`] preserves the existing `value # comment` behavior for
    /// scalars and aliases. [`CommentPosition::Above`] emits the comment on its own line
    /// immediately before the wrapped value. Comments remain suppressed in flow-style
    /// collections in both modes.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub comment_position: CommentPosition,

    /// When enabled, emit `%YAML 1.2` at the beginning of the document and
    /// use YAML 1.2 rules for certain compatibility heuristics.
    ///
    /// In particular, YAML 1.1 boolean spellings like `yes`/`no`/`on`/`off`/`y`/`n`
    /// will **not** be treated as booleans for the purpose of auto-quoting. In cases
    /// like multiple x, y coordinates quoting y may be very annoying.
    /// Default: false.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub yaml_12: bool,

    /// Controls how null values are emitted.
    ///
    /// Defaults to [`NullPolicy::NullNull`]. [`NullPolicy::NullEmpty`] emits empty
    /// scalars in block-style positions, but falls back to `null` in flow-style
    /// collections where an empty scalar would be invalid.
    #[deprecated(
        note = "Direct construction of `SerializerOptions` will be disabled from 1.0.0, use macro `ser_options!`"
    )]
    pub null_policy: NullPolicy,
}

// Below this length, block-string wrappers serialize as regular scalars
// instead of YAML block styles. This keeps short values compact.
pub(crate) const MIN_FOLD_CHARS: usize = 32;
/// Maximum width (in characters) for lines inside folded block scalars.
/// Lines will be wrapped at whitespace so that each emitted line is at most
/// this many characters long (excluding indentation). If no whitespace is
/// available within the limit, the line is not wrapped.
pub(crate) const FOLDED_WRAP_CHARS: usize = 80;

impl SerializerOptions {
    #[allow(deprecated)]
    pub(crate) fn consistent(&self) -> Result<(), Error> {
        if self.indent_step == 0 {
            return Err(Error::InvalidOptions(
                "Invalid indent step must be positive".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for SerializerOptions {
    #[allow(deprecated)]
    fn default() -> Self {
        // Defaults mirror internal constants used by the serializer.
        Self {
            indent_step: 2,
            compact_list_indent: true,
            anchor_generator: None,
            min_fold_chars: MIN_FOLD_CHARS,
            folded_wrap_chars: FOLDED_WRAP_CHARS,
            tagged_enums: false,
            empty_as_braces: true,
            prefer_block_scalars: true,
            quote_all: false,
            comment_position: CommentPosition::Inline,
            yaml_12: false,
            null_policy: NullPolicy::NullNull,
        }
    }
}
