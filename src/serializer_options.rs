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
    /// If true, empty maps are emitted as braces {} and empty lists as []  (this is the default).
    /// Such form is equally valid YAML, allows to tell empty from null and may be easier for a
    /// human to grasp.
    pub empty_as_braces: bool,
    /// Number of spaces to indent per nesting level when emitting block-style collections (2 by default).
    /// 0 value is invalid and will result and error when trying to deserialize, because
    /// no indentation would produce invalid YAML otherwise.
    pub indent_step: usize,
    /// Optional custom anchor-name generator.
    ///
    /// Receives a monotonically increasing `usize` id (starting at 1) and returns the
    /// anchor name to emit. If `None`, the built-in generator yields names like `a1`, `a2`, ...
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
    pub min_fold_chars: usize,
    /// Maximum width (in characters) for lines in folded block scalars (`>`).
    ///
    /// Lines are wrapped **only** at whitespace so that each emitted line is at most
    /// this many characters long (excluding indentation). If no whitespace is present
    /// within the limit (e.g., a single long token), the line is emitted unwrapped
    /// to preserve round-trip correctness: YAML folded scalars typically fold inserted
    /// newlines back as spaces when parsing. 32 default.
    pub folded_wrap_chars: usize,
    /// When enabled, serialize simple enums that become a single scalar (unit variants)
    /// using YAML tags, e.g. `!!Enum Variant` instead of a plain scalar `Variant`.
    /// Deserializer does not need this setting as both cases will be understood. Off by default.
    pub tagged_enums: bool,

    /// When enabled, strings containing more than folded_wrap_chars (80 by default) are written
    /// in wrapped multistring folded form (>), and strings containing new lines are written in
    /// literal form (|), selecting format depending on the number of empty lines at the end.
    /// On by default.
    pub prefer_block_scalars: bool,

    /// When enabled, quote all string scalars. Uses single quotes by default,
    /// but switches to double quotes when the string contains escape sequences
    /// (control characters like `\n`, `\t`, `\r`, backslash) or single quotes.
    /// Disables block scalar styles (`|` and `>`) for quoted strings when active.
    /// Off by default.
    pub quote_all: bool,
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
            anchor_generator: None,
            min_fold_chars: MIN_FOLD_CHARS,
            folded_wrap_chars: FOLDED_WRAP_CHARS,
            tagged_enums: false,
            empty_as_braces: true,
            prefer_block_scalars: true,
            quote_all: false,
        }
    }
}
