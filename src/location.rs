//! Source location utilities.

use saphyr_parser::Span as ParserSpan;
use serde::Deserialize;

/// Type alias for span offset and length fields.
///
/// By default, this is `u32`, which limits offsets/lengths to 4 GiB but keeps [`Span`] compact
/// (4 × `u32` = 16 bytes).
///
/// When the `huge_documents` feature is enabled, this becomes `u64`, allowing documents larger
/// than 4 GiB even on 32-bit platforms, at the cost of increased memory usage
/// (4 × `u64` = 32 bytes).
#[cfg(not(feature = "huge_documents"))]
pub(crate) type SpanIndex = u32;

/// Type alias for span offset and length fields.
///
/// With `huge_documents` enabled, this is `u64`, allowing documents larger than 4 GiB.
/// This increases [`Span`] size from 16 to 32 bytes.
#[cfg(feature = "huge_documents")]
pub(crate) type SpanIndex = u64;

/// A span within the source YAML document.
///
/// This structure provides location information in two forms:
/// 1. **Character-based**: `offset` and `len` count Unicode scalar values. This matches
///    `saphyr-parser`'s native reporting and is always present.
/// 2. **Byte-based**: `byte_info` contains `(byte_offset, byte_len)` counting raw bytes (UTF-8 code units).
///    These are only populated when parsing from string inputs (`&str`, `String`).
///    Byte base indices are internally limited to 32 bits by default (4 Gb documents). If you work
///    with larger YAML documents, enable the `huge_documents` feature or do not use byte
///    offsets (parsing and normal error reporting will still work).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct Span {
    /// Character offset within the source YAML document.
    pub(crate) offset: SpanIndex,
    /// Character length within the source YAML document.
    pub(crate) len: SpanIndex,
    /// Byte offset and length within the source YAML document (offset, len).
    /// Only available when parsing from string sources. `(0, 0)` means unavailable.
    pub(crate) byte_info: (SpanIndex, SpanIndex),
}

impl Span {
    /// Sentinel span meaning "unknown".
    pub const UNKNOWN: Self = Self {
        offset: 0,
        len: 0,
        byte_info: (0, 0),
    };

    /// Returns the character offset within the source YAML document.
    #[inline]
    pub fn offset(&self) -> u64 {
        #[cfg(not(feature = "huge_documents"))]
        {
            self.offset as u64
        }
        #[cfg(feature = "huge_documents")]
        {
            self.offset
        }
    }

    /// Returns the character length within the source YAML document.
    #[inline]
    pub fn len(&self) -> u64 {
        #[cfg(not(feature = "huge_documents"))]
        {
            self.len as u64
        }
        #[cfg(feature = "huge_documents")]
        {
            self.len
        }
    }

    /// Returns the byte offset within the source YAML document.
    /// Returns `None` if byte info is unavailable.
    #[inline]
    pub fn byte_offset(&self) -> Option<u64> {
        if self.byte_info == (0, 0) {
            None
        } else {
            #[cfg(not(feature = "huge_documents"))]
            {
                Some(self.byte_info.0 as u64)
            }
            #[cfg(feature = "huge_documents")]
            {
                Some(self.byte_info.0)
            }
        }
    }

    /// Returns the byte length within the source YAML document.
    /// Returns `None` if byte info is unavailable.
    #[inline]
    pub fn byte_len(&self) -> Option<u64> {
        if self.byte_info == (0, 0) {
            None
        } else {
            #[cfg(not(feature = "huge_documents"))]
            {
                Some(self.byte_info.1 as u64)
            }
            #[cfg(feature = "huge_documents")]
            {
                Some(self.byte_info.1)
            }
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the raw offset value as stored (either `u32` or `u64` depending on the
    /// `huge_documents` feature).
    #[inline]
    pub(crate) fn raw_offset(&self) -> SpanIndex {
        self.offset
    }

    /// Returns the raw length value as stored (either `u32` or `u64` depending on the
    /// `huge_documents` feature).
    #[inline]
    pub(crate) fn raw_len(&self) -> SpanIndex {
        self.len
    }

    /// Returns the raw byte_info tuple as stored.
    #[inline]
    pub(crate) fn raw_byte_info(&self) -> (SpanIndex, SpanIndex) {
        self.byte_info
    }
}

/// Row/column location within the source YAML document (1-indexed, character-based).
///
/// This type is used for both:
/// - deserialization error reporting ([`crate::Error`])
/// - span-aware values ([`crate::Spanned`])
///
/// # Example
///
/// ```
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug)]
/// struct Doc {
///     val: String,
/// }
///
/// // 1. Parse invalid YAML (type mismatch: expected string, found sequence)
/// // Due emoji character and byte offsets are different.
/// let yaml = "val🔑: [1, 2]";
/// let err: Result<Doc, _> = serde_saphyr::from_str(yaml);
///
/// // 2. Obtain the error and its location
/// if let Err(e) = err {
///     if let Some(loc) = e.location() {
///         // 3. Print row, column, and byte offsets
///         // Output: Error at line 1, col 7. Byte offset: 9
///         println!("Error at line {}, col {}", loc.line(), loc.column());
///         if let Some(byte_off) = loc.span().byte_offset() {
///             println!("Byte offset: {}", byte_off);
///         }
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Location {
    /// 1-indexed row number in the input stream.
    pub(crate) line: u32,
    /// 1-indexed column number in the input stream.
    pub(crate) column: u32,
    /// Character-based span within the document
    /// Byte offsets are available for string source but not from the reader.
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

    /// Character-based span within the source document.
    /// For string source, it also can provide byte offsets.
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
///
/// The resulting [`Location::span`] carries character offsets/lengths (not bytes),
/// matching what the parser reports.
pub(crate) fn location_from_span(span: &ParserSpan) -> Location {
    let start = &span.start;
    let end = &span.end;

    let byte_info = if let (Some(start_byte), Some(end_byte)) = (start.byte_offset(), end.byte_offset()) {
        #[cfg(not(feature = "huge_documents"))]
        {
            let len = end_byte.saturating_sub(start_byte);
            // If byte offsets exceed 4 GiB on non-huge builds, mark byte info as unavailable.
            if start_byte > (u32::MAX as usize) || len > (u32::MAX as usize) {
                (0, 0)
            } else {
                (start_byte as SpanIndex, len as SpanIndex)
            }
        }
        #[cfg(feature = "huge_documents")]
        {
            (start_byte as SpanIndex, (end_byte - start_byte) as SpanIndex)
        }
    } else {
        (0, 0)
    };
    
    Location::new(start.line(), start.col() + 1).with_span(Span {
        offset: start.index() as SpanIndex,
        len: span.len() as SpanIndex,
        byte_info,
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
