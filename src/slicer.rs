//! Helpers for proving and computing zero-copy slices into the original YAML input.
//!
//! ## Why this exists
//!
//! The parser (`saphyr_parser`) provides a `Span` that we store in [`crate::Location`].
//! In this crate that span is treated as *character-based* (Unicode scalar values),
//! which is great for user-facing diagnostics.
//!
//! Rust string slicing (`&input[a..b]`) however requires **byte offsets** at UTF-8
//! boundaries.
//!
//! This module centralizes the logic used by the event adapter to decide whether a
//! scalar value can be represented as a verbatim subslice of the original `&str`.
//! If it can, we compute and store **byte offsets** so later deserialization into
//! `&str` is O(1) and truly borrows from the input.
//!
//! ## Safety / correctness model
//!
//! We deliberately *do not* blindly convert character spans into byte spans.
//! Converting chars→bytes is appropriate for diagnostics (see `src/miette.rs`), but
//! it does **not** prove that a parsed scalar exists verbatim in the input.
//!
//! For borrowing we require proof. We accept borrowing only when we can demonstrate
//! that the returned scalar bytes match some slice of the original input.

use std::borrow::Cow;

use saphyr_parser::ScalarStyle;

use crate::de_error::TransformReason;
use crate::location::Location;
use crate::tags::SfTag;

/// Try to compute byte offsets `(start, end)` into `input` that represent the scalar `val`.
///
/// Returns `Ok((start, end))` only when we can *prove* the scalar bytes exist verbatim in
/// the original `input`. Otherwise returns a [`TransformReason`] explaining why borrowing
/// cannot be claimed.
///
/// This is used to implement zero-copy deserialization into `&str`.
pub(crate) fn scalar_borrow_offsets(
    input: Option<&str>,
    val: &Cow<'_, str>,
    style: ScalarStyle,
    tag: SfTag,
    location: &Location,
) -> Result<(usize, usize), TransformReason> {
    let Some(input) = input else {
        // Reader/stream-based parsing does not have a single backing `&str` to borrow from.
        return Err(TransformReason::InputNotBorrowable);
    };

    // Non-stringy tags can imply schema conversion; don't claim we can borrow.
    if !matches!(tag, SfTag::None | SfTag::String | SfTag::NonSpecific) {
        // This is a conservative, "generic" reason that already exists in error reporting.
        return Err(TransformReason::BlockScalarProcessing);
    }

    // Strategy 1 (robust span path): interpret our stored span as character offsets (Unicode
    // scalar values) and convert it to byte offsets in the original UTF-8 `input`.
    //
    // This matches the invariants documented in `src/location.rs` and works for non-ASCII.
    let span = location.span();
    let start_chars = span.offset();
    let end_chars = start_chars.saturating_add(span.len());
    let as_ref = val.as_ref();

    fn byte_index_at_char(s: &str, char_index: usize) -> Option<usize> {
        if char_index == 0 {
            return Some(0);
        }

        // `char_index` is counted in Unicode scalar values.
        s.char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .or_else(|| {
                if char_index == s.chars().count() {
                    Some(s.len())
                } else {
                    None
                }
            })
    }

    if let (Some(start), Some(end)) = (
        byte_index_at_char(input, start_chars),
        byte_index_at_char(input, end_chars),
    ) {
        if start <= end && end <= input.len() {
            let raw = &input[start..end];

            // Common case: plain scalars where parser span covers exactly the scalar bytes.
            if raw == as_ref {
                return Ok((start, end));
            }

            // Common case: quoted scalars where the span includes quotes but `val` excludes them.
            if matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted)
                && raw.len() >= 2
                && &raw[1..raw.len() - 1] == as_ref
            {
                return Ok((start + 1, end - 1));
            }
        }
    }

    // Strategy 2 (robust path): when the parser itself returned `Cow::Borrowed`, it is
    // already a subslice of some backing buffer. If that backing buffer is our `input`, we
    // can compute byte offsets by pointer arithmetic.
    //
    // This does *not* rely on any span interpretation.
    if let Cow::Borrowed(borrowed) = val {
        let base = input.as_bytes().as_ptr() as usize;
        let ptr = borrowed.as_bytes().as_ptr() as usize;
        let len = borrowed.len();
        let input_len = input.len();

        // Ensure `borrowed` points into `input`.
        if ptr >= base
            && ptr
                .checked_add(len)
                .is_some_and(|end| end <= base + input_len)
        {
            let start = ptr - base;
            let end = start + len;
            if input.is_char_boundary(start) && input.is_char_boundary(end) {
                return Ok((start, end));
            }

            // Should be unreachable for correct UTF-8 slices, but keep a conservative reason.
            return Err(TransformReason::MultiLineNormalization);
        }

        return Err(TransformReason::InputNotBorrowable);
    }

    // Could not prove that the returned string exists verbatim in the input.
    // Keep the same style-based rejection reasons as the original inline implementation.
    let reason = match style {
        ScalarStyle::Plain => TransformReason::MultiLineNormalization,
        ScalarStyle::SingleQuoted => TransformReason::SingleQuoteEscape,
        ScalarStyle::DoubleQuoted => TransformReason::EscapeSequence,
        ScalarStyle::Folded => TransformReason::LineFolding,
        ScalarStyle::Literal => TransformReason::BlockScalarProcessing,
    };
    Err(reason)
}
