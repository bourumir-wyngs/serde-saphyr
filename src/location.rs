//! Source location utilities.

use saphyr_parser::Span;
use serde::Deserialize;

/// Row/column location within the source YAML document (1-indexed).
///
/// This type is used for both:
/// - deserialization error reporting ([`crate::Error`])
/// - span-aware values ([`crate::Spanned`])
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Location {
    /// 1-indexed row number in the input stream.
    pub(crate) line: u32,
    /// 1-indexed column number in the input stream.
    pub(crate) column: u32,
}

impl Location {
    /// serde_yaml-compatible line information.
    #[inline]
    pub fn line(&self) -> u64 {
        self.line as u64
    }

    /// serde_yaml-compatible column information.
    #[inline]
    pub fn column(&self) -> u64 {
        self.column as u64
    }
}

impl Location {
    /// Sentinel value meaning "location unknown".
    ///
    /// Used when a precise position is not yet available at error creation time.
    pub const UNKNOWN: Self = Self { line: 0, column: 0 };

    /// Create a new location record.
    ///
    /// Arguments:
    /// - `line`: 1-indexed line.
    /// - `column`: 1-indexed column.
    pub(crate) const fn new(line: usize, column: usize) -> Self {
        // 4 Gb is larger than any YAML document I can imagine, and also this is
        // error reporting only.
        Self {
            line: line as u32,
            column: column as u32,
        }
    }
}

/// Convert a `saphyr_parser::Span` to a 1-indexed [`Location`].
///
/// Called by:
/// - The live events adapter for each raw parser event.
pub(crate) fn location_from_span(span: &Span) -> Location {
    let start = &span.start;
    Location::new(start.line(), start.col() + 1)
}
