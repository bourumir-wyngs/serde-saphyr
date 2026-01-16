//! Source location utilities.

use saphyr_parser::Span as ParserSpan;
use serde::Deserialize;

/// A byte span within the source YAML document.
///
/// This is intended for future first-class `miette` integration, which is
/// primarily offset/length based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct Span {
    /// Byte offset within the source YAML document.
    pub(crate) offset: usize,
    /// Byte length within the source YAML document.
    pub(crate) len: u32,
}

impl Span {
    /// Sentinel span meaning "unknown".
    pub const UNKNOWN: Self = Self { offset: 0, len: 0 };

    /// Returns the byte offset within the source YAML document.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the byte length within the source YAML document.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

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
    /// Byte span within the document.
    #[serde(default)]
    pub(crate) span: Span,
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

    /// Byte span within the source document.
    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }
}

impl Location {
    /// Sentinel value meaning "location unknown".
    ///
    /// Used when a precise position is not yet available at error creation time.
    pub const UNKNOWN: Self = Self {
        line: 0,
        column: 0,
        span: Span::UNKNOWN,
    };

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
            span: Span::UNKNOWN,
        }
    }

    pub(crate) const fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

/// Convert a `saphyr_parser::Span` to a 1-indexed [`Location`].
///
/// Called by:
/// - The live events adapter for each raw parser event.
pub(crate) fn location_from_span(span: &ParserSpan) -> Location {
    let start = &span.start;
    Location::new(start.line(), start.col() + 1).with_span(Span {
        offset: start.index(),
        len: span.len() as u32,
    })
}

/// Pair of locations for values that may come indirectly from YAML anchors.
///
/// - `reference_location`: where the value is *used* (alias/merge site).
/// - `defined_location`: where the value is *defined* (anchor definition site).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Locations {
    pub reference_location: Location,
    pub defined_location: Location,
}

impl Locations {
    #[cfg_attr(not(any(feature = "garde", feature = "validator")), allow(dead_code))]
    pub(crate) const UNKNOWN: Locations = Locations {
        reference_location: Location::UNKNOWN,
        defined_location: Location::UNKNOWN,
    };

    #[inline]
    pub(crate) fn same(location: &Location) -> Option<Locations> {
        if location == &Location::UNKNOWN {
            None
        } else {
            Some(Locations {
                reference_location: *location,
                defined_location: *location,
            })
        }
    }

    #[inline]
    pub fn primary_location(self) -> Option<Location> {
        if self.reference_location != Location::UNKNOWN {
            Some(self.reference_location)
        } else if self.defined_location != Location::UNKNOWN {
            Some(self.defined_location)
        } else {
            None
        }
    }
}
