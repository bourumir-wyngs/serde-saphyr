use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use std::fmt::Write;

use super::helpers::{BoolCapture, StrCapture, UsizeCapture, scalar_key_to_string};
use super::{AnchorId, YamlSerializer};
use crate::ser::options::CommentPosition;
use crate::ser::{Error, Result};

/// From the spec:
///
/// > If the "?" indicator is omitted, parsing needs to see past the implicit key to recognize it as such.
/// > To limit the amount of lookahead required, the ":" indicator must appear at most 1024 Unicode characters beyond the start of the key.
/// > In addition, the key is restricted to a single line.
const SIMPLE_KEY_MAX_LEN: usize = 1024;

// ------------------------------------------------------------
// Seq / Tuple serializers
// ------------------------------------------------------------

/// Serializer for sequences and tuples.
///
/// Created by `YamlSerializer::serialize_seq`/`serialize_tuple`. Holds a mutable
/// reference to the parent serializer and formatting state for the sequence.
pub struct SeqSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    pub(super) ser: &'a mut YamlSerializer<'b, W>,
    /// Target indentation depth for block-style items.
    pub(super) depth: usize,
    /// Whether the sequence is being written in flow style (`[a, b]`).
    pub(super) flow: bool,
    /// Whether the next element is the first (comma handling in flow style).
    pub(super) first: bool,
}

impl<'a, 'b, W: Write> SerializeTuple for SeqSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<()> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<()> {
        SerializeSeq::end(self)
    }
}

// Re-implement SerializeSeq for SeqSer with correct end.
impl<'a, 'b, W: Write> SerializeSeq for SeqSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<()> {
        if self.flow {
            if !self.first {
                self.ser.out.write_str(", ")?;
            }
            self.ser.with_in_flow(|s| v.serialize(s))?;
        } else {
            // If we are the value of a mapping key, we deferred the newline until we knew the
            // sequence is non-empty. Insert it now before emitting the first dash.
            if self.first && self.ser.pending_space_after_colon {
                self.ser.pending_space_after_colon = false;
                if !self.ser.at_line_start {
                    self.ser.newline()?;
                }
            }
            // If previous element was an inline map after a dash, just clear the flag; do not change depth.
            if !self.first && self.ser.inline_map_after_dash {
                self.ser.inline_map_after_dash = false;
            }
            if self.first && (!self.ser.at_line_start || self.ser.pending_inline_map) {
                // Inline the first element of this nested sequence right after the outer dash
                // (either we are already mid-line, or the parent staged inline via pending_inline_map).
                // Do not write indentation here.
            } else {
                self.ser.write_indent(self.depth)?;
            }
            self.ser.out.write_str("- ")?;
            self.ser.at_line_start = false;
            if self.first && self.ser.inline_map_after_dash {
                // We consumed the inline-after-dash behavior for this child sequence.
                self.ser.inline_map_after_dash = false;
            }
            // Capture the dash's indentation depth for potential struct-variant that follows.
            self.ser.after_dash_depth = Some(self.depth);
            // Hint to emit first key/element of a following mapping/sequence inline on the same line.
            self.ser.pending_inline_map = true;
            // A sibling element's block-collection state must not leak into this element.
            self.ser.last_value_was_block = false;
            v.serialize(&mut *self.ser)?;
        }
        self.first = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.flow {
            self.ser.out.write_str("]")?;
            if self.ser.in_flow == 0 {
                self.ser.newline()?;
            }
        } else if self.first {
            // Empty block-style sequence.
            if self.ser.empty_as_braces {
                // If we were pending a space after a colon (map value position), write it now.
                if self.ser.pending_space_after_colon {
                    self.ser.out.write_str(" ")?;
                    self.ser.pending_space_after_colon = false;
                } else if self.ser.at_line_start {
                    // If at line start, indent appropriately.
                    self.ser.write_indent(self.depth)?;
                }
                self.ser.out.write_str("[]")?;
                self.ser.newline()?;
            } else {
                // Preserve legacy behavior: just emit a newline (empty body).
                // Clear map-value pending state so it does not leak into following elements.
                self.ser.pending_space_after_colon = false;
                self.ser.newline()?;
            }
        } else {
            // Block collection finished and it was not empty.
            self.ser.last_value_was_block = true;
            // Clear any dash/inline hints so they cannot affect the next sibling value
            // (e.g., a mapping field following a block sequence value).
            self.ser.pending_inline_map = false;
            self.ser.after_dash_depth = None;
            self.ser.inline_map_after_dash = false;
        }
        Ok(())
    }
}

/// Serializer for tuple-structs.
///
/// Used for three shapes:
/// - Normal tuple-structs (treated like sequences in block style),
/// - Anchors:
///   - Internal strong-anchor payloads (`__yaml_anchor`),
///   - Internal weak-anchor payloads (`__yaml_weak_anchor`).
pub enum TupleSer<'a, 'b, W: Write> {
    /// Normal tuple-struct: a block sequence.
    Seq(SeqSer<'a, 'b, W>),
    /// Anchor/comment wrapper payload.
    Special(SpecialTupleSer<'a, 'b, W>),
}

/// State machine for the internal anchor/comment tuple wrappers.
pub struct SpecialTupleSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSerializer<'b, W>,
    /// Variant describing how to interpret fields.
    kind: TupleKind,
    /// Current field index being serialized.
    idx: usize,
    /// Captured pointer identity for a weak-anchor payload while waiting for
    /// its `present` field.
    weak_anchor_ptr: usize,

    // ---- Extra fields for refactoring/perf/correctness ----
    /// For strong anchors: if Some(id) then we must emit an alias instead of a definition at field #2.
    strong_alias_id: Option<AnchorId>,
    /// For weak anchors: whether the `present` flag was true.
    weak_present: bool,
    /// Skip serializing the 3rd field (value) in weak case if present==false.
    skip_third: bool,
    /// For weak anchors: hold alias id if value should be emitted as alias in field #3.
    weak_alias_id: Option<AnchorId>,
    /// For commented wrapper: captured comment text from field #0.
    comment_text: Option<String>,
}
enum TupleKind {
    AnchorStrong, // [ptr, value]
    AnchorWeak,   // [ptr, present, value]
    Commented,    // [comment, value]
}
impl<'a, 'b, W: Write> TupleSer<'a, 'b, W> {
    /// Create a tuple serializer for internal strong-anchor payloads.
    pub(super) fn anchor_strong(ser: &'a mut YamlSerializer<'b, W>) -> Self {
        TupleSer::Special(SpecialTupleSer::new(ser, TupleKind::AnchorStrong))
    }
    /// Create a tuple serializer for internal weak-anchor payloads.
    pub(super) fn anchor_weak(ser: &'a mut YamlSerializer<'b, W>) -> Self {
        TupleSer::Special(SpecialTupleSer::new(ser, TupleKind::AnchorWeak))
    }
    /// Create a tuple serializer for internal commented wrapper.
    pub(super) fn commented(ser: &'a mut YamlSerializer<'b, W>) -> Self {
        TupleSer::Special(SpecialTupleSer::new(ser, TupleKind::Commented))
    }
}

impl<'a, 'b, W: Write> SpecialTupleSer<'a, 'b, W> {
    fn new(ser: &'a mut YamlSerializer<'b, W>, kind: TupleKind) -> Self {
        Self {
            ser,
            kind,
            idx: 0,
            weak_anchor_ptr: 0,
            strong_alias_id: None,
            weak_present: false,
            skip_third: false,
            weak_alias_id: None,
            comment_text: None,
        }
    }
}

impl<'a, 'b, W: Write> SerializeTupleStruct for TupleSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        match self {
            TupleSer::Seq(seq) => SerializeSeq::serialize_element(seq, value),
            TupleSer::Special(s) => s.serialize_field(value),
        }
    }

    fn end(self) -> Result<()> {
        match self {
            TupleSer::Seq(seq) => SerializeSeq::end(seq),
            TupleSer::Special(s) => s.end(),
        }
    }
}

impl<'a, 'b, W: Write> SpecialTupleSer<'a, 'b, W> {
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        match self.kind {
            TupleKind::AnchorStrong => {
                match self.idx {
                    0 => {
                        // capture ptr, decide define vs alias
                        let mut cap = UsizeCapture::default();
                        value.serialize(&mut cap)?;
                        let ptr = cap.finish()?;
                        let (id, fresh) = self.ser.alloc_anchor_for(ptr);
                        if fresh {
                            self.ser.pending_anchor_id = Some(id); // define before value
                            self.strong_alias_id = None;
                        } else {
                            self.strong_alias_id = Some(id); // alias instead of value
                        }
                    }
                    1 => {
                        if let Some(id) = self.strong_alias_id.take() {
                            // Already defined earlier -> emit alias
                            self.ser.write_alias_id(id)?;
                        } else {
                            // First sight -> serialize value; pending_anchor_id (if any) will be emitted
                            value.serialize(&mut *self.ser)?;
                        }
                    }
                    _ => return Err(Error::unexpected("unexpected field in __yaml_anchor")),
                }
            }
            TupleKind::AnchorWeak => {
                match self.idx {
                    0 => {
                        let mut cap = UsizeCapture::default();
                        value.serialize(&mut cap)?;
                        let ptr = cap.finish()?;
                        self.weak_anchor_ptr = ptr;
                    }
                    1 => {
                        let mut bc = BoolCapture::default();
                        value.serialize(&mut bc)?;
                        self.weak_present = bc.finish()?;
                        if !self.weak_present {
                            // present == false: emit null and skip field #3
                            if self.ser.at_line_start {
                                self.ser.write_indent(self.ser.depth)?;
                            }
                            self.ser.out.write_str("null")?;
                            // Use shared end-of-scalar so pending inline comments (if any) are appended
                            self.ser.write_end_of_scalar()?;
                            self.skip_third = true;
                        } else {
                            let ptr = self.weak_anchor_ptr;
                            let (id, fresh) = self.ser.alloc_anchor_for(ptr);
                            if fresh {
                                self.ser.pending_anchor_id = Some(id); // define before value
                                self.weak_alias_id = None;
                            } else {
                                self.weak_alias_id = Some(id); // alias in field #3
                            }
                        }
                    }
                    2 => {
                        if self.skip_third {
                            // nothing to do
                        } else if let Some(id) = self.weak_alias_id.take() {
                            self.ser.write_alias_id(id)?;
                        } else {
                            // definition path: pending_anchor_id (if any) will be placed automatically
                            value.serialize(&mut *self.ser)?;
                        }
                    }
                    _ => return Err(Error::unexpected("unexpected field in __yaml_weak_anchor")),
                }
            }
            TupleKind::Commented => {
                match self.idx {
                    0 => {
                        // Capture comment string
                        let mut sc = StrCapture::default();
                        value.serialize(&mut sc)?;
                        self.comment_text = Some(sc.finish()?);
                    }
                    1 => {
                        let comment = self.comment_text.take().unwrap_or_default();
                        let sanitized = YamlSerializer::<W>::sanitize_comment_text(&comment);
                        match self.ser.comment_position {
                            CommentPosition::Inline => {
                                if self.ser.in_flow == 0 && !sanitized.is_empty() {
                                    // Stage the comment so scalar/alias serializers append it inline via write_end_of_scalar.
                                    self.ser.pending_inline_comment = Some(sanitized);
                                }
                                // Serialize the inner value as-is. Complex values will ignore the comment (it will be cleared).
                                value.serialize(&mut *self.ser)?;
                                // Ensure no leftover staged comment leaks to subsequent tokens.
                                self.ser.pending_inline_comment = None;
                            }
                            CommentPosition::Above => {
                                let saved_depth = self.ser.depth;
                                let target_depth = self.ser.write_above_comment(&sanitized)?;
                                if let Some(depth) = target_depth {
                                    self.ser.depth = depth;
                                }
                                let result = value.serialize(&mut *self.ser);
                                self.ser.depth = saved_depth;
                                result?;
                                self.ser.pending_inline_comment = None;
                            }
                        }
                    }
                    _ => return Err(Error::unexpected("unexpected field in __yaml_commented")),
                }
            }
        }
        self.idx += 1;
        Ok(())
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// Tuple variant (enum Variant: ( ... )).
// `serialize_tuple_variant` writes the variant name and colon, then hands the
// fields to a `SeqSer`: the body is just a block sequence, so it reuses the same
// dash/indentation logic as `serialize_seq`.
impl<'a, 'b, W: Write> SerializeTupleVariant for SeqSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        SerializeSeq::end(self)
    }
}

// ------------------------------------------------------------
// Map / Struct serializers
// ------------------------------------------------------------

/// Serializer for maps and structs.
///
/// Created by `YamlSerializer::serialize_map`/`serialize_struct`. Manages indentation
/// and flow/block style for key-value pairs.
pub struct MapSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    pub(super) ser: &'a mut YamlSerializer<'b, W>,
    /// Target indentation depth for block-style entries.
    pub(super) depth: usize,
    /// Whether the mapping is in flow style (`{k: v}`).
    pub(super) flow: bool,
    /// Whether the next entry is the first (comma handling in flow style).
    pub(super) first: bool,
    /// Whether the most recently serialized key was a complex (non-scalar) node.
    pub(super) last_key_complex: bool,
    /// Align continuation lines under an inline-after-dash first key by adding 2 spaces.
    pub(super) align_after_dash: bool,
    /// If true, this mapping began in a value position and stayed inline (after `key:`)
    /// so that an empty map can be serialized as `{}` right there. When the first key arrives,
    /// we must break the line and indent appropriately.
    pub(super) inline_value_start: bool,
}

impl<'a, 'b, W: Write> MapSer<'a, 'b, W> {
    /// Emit a scalar key inline as `key:`.
    fn write_simple_key(&mut self, text: &str) -> Result<()> {
        // Indent continuation lines. If this map started inline after a dash,
        // align under the first key by adding two spaces instead of a full indent step.
        if self.align_after_dash && self.ser.at_line_start {
            let base = self.depth.saturating_sub(1);
            for _ in 0..self.ser.indent_step * base {
                self.ser.out.write_char(' ')?;
            }
            self.ser.out.write_str("  ")?; // width of "- "
            self.ser.at_line_start = false;
        } else {
            self.ser.write_indent(self.depth)?;
        }
        self.ser.out.write_str(text)?;
        self.ser.out.write_str(":")?;
        self.ser.pending_space_after_colon = true;
        self.ser.at_line_start = false;
        self.last_key_complex = false;
        Ok(())
    }

    /// Emit a key using the explicit `? key` form; the value follows on a `: value` line.
    fn write_explicit_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        self.ser.write_anchor_for_complex_node()?;
        self.ser.write_indent(self.depth)?;
        self.ser.out.write_str("? ")?;
        self.ser.at_line_start = false;

        let saved_depth = self.ser.depth;
        let saved_current_map_depth = self.ser.current_map_depth;
        let saved_pending_inline_map = self.ser.pending_inline_map;
        let saved_inline_map_after_dash = self.ser.inline_map_after_dash;
        let saved_after_dash_depth = self.ser.after_dash_depth;

        self.ser.pending_inline_map = true;
        self.ser.depth = self.depth;
        // Provide a base depth for nested maps within this complex key so that
        // continuation lines indent one level deeper than the parent mapping.
        self.ser.current_map_depth = Some(self.depth);
        self.ser.after_dash_depth = None;
        key.serialize(&mut *self.ser)?;

        self.ser.depth = saved_depth;
        self.ser.current_map_depth = saved_current_map_depth;
        self.ser.pending_inline_map = saved_pending_inline_map;
        self.ser.inline_map_after_dash = saved_inline_map_after_dash;
        self.ser.after_dash_depth = saved_after_dash_depth;
        // A complex key may have been serialized as a block collection, which sets
        // `last_value_was_block`. That state must NOT affect the *value* of this same
        // map entry (e.g. we still want `: x: 3` inline for composite-key maps).
        self.ser.last_value_was_block = false;
        self.last_key_complex = true;
        Ok(())
    }
}

impl<'a, 'b, W: Write> SerializeMap for MapSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        if self.flow {
            if !self.first {
                self.ser.out.write_str(", ")?;
            }
            let text = scalar_key_to_string(key, self.ser.yaml_12)?;
            self.ser.out.write_str(&text)?;
            self.ser.out.write_str(": ")?;
            self.ser.at_line_start = false;
            self.last_key_complex = false;
        } else {
            // If this mapping started inline as a value (after "key:"), but now we
            // are about to emit the first entry, move to the next line before the key.
            if self.inline_value_start {
                // Cancel a pending space after ':' and break the line.
                if self.ser.pending_space_after_colon {
                    self.ser.pending_space_after_colon = false;
                }
                if !self.ser.at_line_start {
                    self.ser.newline()?;
                }
                self.inline_value_start = false;
            } else if !self.ser.at_line_start {
                self.ser.write_space_if_pending()?;
            }

            // A new key in a block map should clear any pending inline hints from previous siblings.
            self.ser.after_dash_depth = None;
            self.ser.pending_inline_map = false;
            self.ser.last_value_was_block = false;

            match scalar_key_to_string(key, self.ser.yaml_12) {
              Ok(text)
              // since a utf8 "character" is at min one byte, text.len() is an cheap upper bound on the number of chars
                    if text.len() <= SIMPLE_KEY_MAX_LEN
                        || text.chars().count() <= SIMPLE_KEY_MAX_LEN =>
                {
                    self.write_simple_key(&text)?;
                }
                Ok(_) => self.write_explicit_key(key)?,
                Err(Error::Unexpected { msg }) if msg == "non-scalar key" => {
                    self.write_explicit_key(key)?;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        if self.flow {
            self.ser.with_in_flow(|s| value.serialize(s))?;
        } else {
            let saved_pending_inline_map = self.ser.pending_inline_map;
            let saved_depth = self.ser.depth;
            if self.last_key_complex {
                if self.align_after_dash && self.ser.at_line_start {
                    let base = self.depth.saturating_sub(1);
                    for _ in 0..self.ser.indent_step * base {
                        self.ser.out.write_char(' ')?;
                    }
                    self.ser.out.write_str("  ")?;
                    self.ser.at_line_start = false;
                } else {
                    self.ser.write_indent(self.depth)?;
                }
                self.ser.out.write_str(":")?;
                self.ser.pending_space_after_colon = true;
                self.ser.pending_inline_map = true;
                self.ser.at_line_start = false;
                self.ser.depth = self.depth;
            }
            let prev_map_depth = self.ser.current_map_depth.replace(self.depth);
            let result = value.serialize(&mut *self.ser);
            self.ser.current_map_depth = prev_map_depth;
            // Always restore the parent's pending_inline_map to avoid leaking inline hints
            // across sibling values (e.g., after finishing a sequence value like `groups`).
            self.ser.pending_inline_map = saved_pending_inline_map;
            if self.last_key_complex {
                self.ser.depth = saved_depth;
                self.last_key_complex = false;
            }
            result?;
        }
        self.first = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.flow {
            self.ser.out.write_str("}")?;
            if self.ser.in_flow == 0 {
                self.ser.newline()?;
            }
        } else if self.first {
            // Empty block-style map.
            if self.ser.empty_as_braces {
                // If we were pending a space after a colon (map value position), write it now.
                if self.ser.pending_space_after_colon {
                    self.ser.out.write_str(" ")?;
                    self.ser.pending_space_after_colon = false;
                }
                // If at line start, indent appropriately.
                if self.ser.at_line_start {
                    // If we are aligning after a dash, mimic the indentation logic used for keys.
                    if self.align_after_dash {
                        let base = self.depth.saturating_sub(1);
                        for _ in 0..self.ser.indent_step * base {
                            self.ser.out.write_char(' ')?;
                        }
                        self.ser.out.write_str("  ")?; // width of "- "
                        self.ser.at_line_start = false;
                    } else {
                        self.ser.write_indent(self.depth)?;
                    }
                }
                self.ser.out.write_str("{}")?;
                self.ser.newline()?;
            } else {
                // Preserve legacy behavior: just emit a newline (empty body).
                self.ser.newline()?;
            }
        } else {
            // Block collection finished and it was not empty.
            self.ser.last_value_was_block = true;
        }
        Ok(())
    }
}
impl<'a, 'b, W: Write> SerializeStruct for MapSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        SerializeMap::serialize_key(self, &key)?;
        SerializeMap::serialize_value(self, value)
    }
    fn end(self) -> Result<()> {
        SerializeMap::end(self)
    }
}

/// Serializer for struct variants (enum Variant: { ... }).
///
/// Created by `YamlSerializer::serialize_struct_variant` to emit the variant name
/// followed by a block mapping of fields.
pub struct StructVariantSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    pub(super) ser: &'a mut YamlSerializer<'b, W>,
    /// Target indentation depth for the fields.
    pub(super) depth: usize,
}
impl<'a, 'b, W: Write> SerializeStructVariant for StructVariantSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        let text = scalar_key_to_string(&key, self.ser.yaml_12)?;
        self.ser.write_indent(self.depth)?;
        self.ser.out.write_str(&text)?;
        // Defer spacing/newline decision to the value serializer similarly to map entries.
        self.ser.out.write_str(":")?;
        self.ser.pending_space_after_colon = true;
        self.ser.at_line_start = false;
        // Ensure nested mappings/collections used as this field's value indent relative to this struct variant.
        let prev_map_depth = self.ser.current_map_depth.replace(self.depth);
        let result = value.serialize(&mut *self.ser);
        self.ser.current_map_depth = prev_map_depth;
        result
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}
