use serde::Deserialize;
#[cfg(feature = "huge_documents")]
use serde::Deserializer;
#[cfg(feature = "huge_documents")]
use std::fmt;

#[cfg(not(feature = "huge_documents"))]
pub(crate) type SpanIndex = u32;

#[cfg(feature = "huge_documents")]
const MAX_PACKED_SPAN_INDEX: u64 = (1u64 << 48) - 1;

#[cfg(feature = "huge_documents")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct SpanIndex([u8; 6]);

#[cfg(feature = "huge_documents")]
impl SpanIndex {
    const fn from_u64_saturating(value: u64) -> Self {
        let value = if value > MAX_PACKED_SPAN_INDEX {
            MAX_PACKED_SPAN_INDEX
        } else {
            value
        };

        Self([
            value as u8,
            (value >> 8) as u8,
            (value >> 16) as u8,
            (value >> 24) as u8,
            (value >> 32) as u8,
            (value >> 40) as u8,
        ])
    }

    const fn to_u64(self) -> u64 {
        (self.0[0] as u64)
            | ((self.0[1] as u64) << 8)
            | ((self.0[2] as u64) << 16)
            | ((self.0[3] as u64) << 24)
            | ((self.0[4] as u64) << 32)
            | ((self.0[5] as u64) << 40)
    }
}

#[cfg(feature = "huge_documents")]
impl fmt::Debug for SpanIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_u64().fmt(f)
    }
}

#[cfg(feature = "huge_documents")]
impl<'de> Deserialize<'de> for SpanIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_u64_saturating(u64::deserialize(deserializer)?))
    }
}

#[cfg(not(feature = "huge_documents"))]
const fn span_index_from_u64_saturating(value: u64) -> SpanIndex {
    if value > (u32::MAX as u64) {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(feature = "huge_documents")]
const fn span_index_from_u64_saturating(value: u64) -> SpanIndex {
    SpanIndex::from_u64_saturating(value)
}

#[cfg(not(feature = "huge_documents"))]
const fn span_index_to_u64(value: SpanIndex) -> u64 {
    value as u64
}

#[cfg(feature = "huge_documents")]
const fn span_index_to_u64(value: SpanIndex) -> u64 {
    value.to_u64()
}

/// A span within the source YAML document.
///
/// This structure provides location information in two forms:
/// 1. **Character-based**: `offset` and `len` count Unicode scalar values. This matches
///    `granit-parser`'s native reporting and is always present.
/// 2. **Byte-based**: `byte_info` contains `(byte_offset, byte_len)` counting raw bytes (UTF-8 code units).
///    These are only populated when parsing from string inputs (`&str`, `String`).
///
/// By default, offsets are stored internally as `u32`, which keeps the span compact and
/// supports YAML inputs up to 4 GiB for byte-based coordinates.
///
/// With the `huge_documents` feature, offsets are stored in a packed 48-bit representation.
/// Public getters still return `u64`, and values beyond 48 bits saturate instead of wrapping.
/// This keeps [`crate::Location`] compact enough to avoid inflating [`crate::Error`] as much
/// as a full `u64`-based layout would.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct Span {
    offset: SpanIndex,
    len: SpanIndex,
    byte_info: (SpanIndex, SpanIndex),
}

impl Span {
    /// Sentinel span meaning "unknown".
    pub const UNKNOWN: Self = Self::new(0, 0);

    /// Construct a span from character-based offset and length.
    ///
    /// Values that exceed the active storage range are saturated.
    pub const fn new(offset: u64, len: u64) -> Self {
        Self {
            offset: span_index_from_u64_saturating(offset),
            len: span_index_from_u64_saturating(len),
            byte_info: (
                span_index_from_u64_saturating(0),
                span_index_from_u64_saturating(0),
            ),
        }
    }

    /// Attach byte-based offset and length information to the span.
    ///
    /// Values that exceed the active storage range are saturated.
    pub const fn with_byte_info(mut self, byte_offset: u64, byte_len: u64) -> Self {
        self.byte_info = (
            span_index_from_u64_saturating(byte_offset),
            span_index_from_u64_saturating(byte_len),
        );
        self
    }

    /// Returns the character offset within the source YAML document.
    #[inline]
    pub fn offset(&self) -> u64 {
        span_index_to_u64(self.offset)
    }

    /// Returns the character length within the source YAML document.
    #[inline]
    pub fn len(&self) -> u64 {
        span_index_to_u64(self.len)
    }

    /// Returns the byte offset within the source YAML document.
    /// Returns `None` if byte info is unavailable.
    #[inline]
    pub fn byte_offset(&self) -> Option<u64> {
        if self.byte_info
            == (
                span_index_from_u64_saturating(0),
                span_index_from_u64_saturating(0),
            )
        {
            None
        } else {
            Some(span_index_to_u64(self.byte_info.0))
        }
    }

    /// Returns the byte length within the source YAML document.
    /// Returns `None` if byte info is unavailable.
    #[inline]
    pub fn byte_len(&self) -> Option<u64> {
        if self.byte_info
            == (
                span_index_from_u64_saturating(0),
                span_index_from_u64_saturating(0),
            )
        {
            None
        } else {
            Some(span_index_to_u64(self.byte_info.1))
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(feature = "deserialize")]
    #[inline]
    pub(crate) fn byte_info_or_zero(&self) -> (u64, u64) {
        (
            span_index_to_u64(self.byte_info.0),
            span_index_to_u64(self.byte_info.1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "huge_documents")]
    use std::mem::size_of;

    #[test]
    fn span_constructors_round_trip_small_values() {
        let span = Span::new(10, 5).with_byte_info(20, 5);
        assert_eq!(span.offset(), 10);
        assert_eq!(span.len(), 5);
        assert_eq!(span.byte_offset(), Some(20));
        assert_eq!(span.byte_len(), Some(5));
    }

    #[test]
    fn unknown_span_has_no_byte_info() {
        assert_eq!(Span::UNKNOWN.byte_offset(), None);
        assert_eq!(Span::UNKNOWN.byte_len(), None);
        assert!(Span::UNKNOWN.is_empty());
    }

    #[cfg(feature = "huge_documents")]
    #[test]
    fn huge_document_indices_saturate_to_48_bits() {
        let span = Span::new(u64::MAX, u64::MAX).with_byte_info(u64::MAX, u64::MAX);
        assert_eq!(span.offset(), MAX_PACKED_SPAN_INDEX);
        assert_eq!(span.len(), MAX_PACKED_SPAN_INDEX);
        assert_eq!(span.byte_offset(), Some(MAX_PACKED_SPAN_INDEX));
        assert_eq!(span.byte_len(), Some(MAX_PACKED_SPAN_INDEX));
    }

    #[cfg(feature = "huge_documents")]
    #[test]
    fn huge_document_layout_stays_compact() {
        assert_eq!(size_of::<Span>(), 24);
        assert!(size_of::<crate::Location>() <= 40);
    }

    #[cfg(all(feature = "huge_documents", feature = "deserialize"))]
    #[test]
    fn huge_document_error_layout_stays_below_clippy_threshold() {
        assert!(size_of::<crate::Error>() < 128);
    }
}
