#[path = "compound.rs"]
mod compound;
#[path = "helpers.rs"]
mod helpers;

pub use self::compound::{MapSer, SeqSer, StructVariantSer, TupleSer};

use self::helpers::StrCapture;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use nohash_hasher::BuildNoHashHasher;
use serde_core::ser::Error as _;
use serde_core::ser::{Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::Write;

use crate::long_strings::{NAME_FOLD_STR, NAME_LIT_STR};

use super::options::{CommentPosition, FOLDED_WRAP_CHARS, MIN_FOLD_CHARS, SerializerOptions};
use super::quoting::{
    is_auto_block_scalar_readable, is_block_scalar_content_safe, is_controll_which_needs_escaping,
    is_plain_safe, is_plain_value_safe,
};
use super::{
    Error, NAME_DOUBLE_QUOTED, NAME_FLOW_MAP, NAME_FLOW_SEQ, NAME_NULLABLE_TILDE,
    NAME_SINGLE_QUOTED, NAME_SPACE_AFTER, NAME_TUPLE_ANCHOR, NAME_TUPLE_COMMENTED, NAME_TUPLE_WEAK,
    Result, wrapping, zmij_format,
};

// ------------------------------------------------------------
// Core serializer
// ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingFlow {
    AnySeq,
    AnyMap,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum StrStyle {
    Literal, // |
    Folded,  // >
}

// Numeric anchor id used internally.
type AnchorId = u32;

/// Core YAML serializer used by `to_string`, `to_fmt_writer`, and `to_io_writer` (and their `_with_options` variants).
///
/// This type implements `serde::Serializer` and writes YAML to a `fmt::Write`.
/// It manages indentation, flow/block styles, and YAML anchors/aliases.
///
/// This type is also re-exported from the crate root as [`serde_saphyr::Serializer`](crate::Serializer).
///
/// ## Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Foo {
///     a: i32,
///     b: bool,
/// }
///
/// let mut out = String::new();
/// let mut ser = serde_saphyr::Serializer::new(&mut out);
/// Foo { a: 1, b: true }.serialize(&mut ser)?;
///
/// assert!(out.contains("a: 1"));
/// # Ok::<(), serde_saphyr::ser::Error>(())
/// ```
pub struct YamlSerializer<'a, W: Write> {
    /// Destination writer where YAML text is emitted.
    out: &'a mut W,
    /// Spaces per indentation level for block-style collections.
    indent_step: usize,
    /// Threshold for downgrading block-string wrappers to plain scalars.
    min_fold_chars: usize,
    /// Wrap width for folded block scalars ('>').
    folded_wrap_col: usize,
    /// Current nesting depth (used for indentation).
    depth: usize,
    /// Whether the cursor is at the start of a line.
    at_line_start: bool,

    // Anchors:
    /// Map from pointer identity to anchor id.
    anchors: HashMap<usize, AnchorId, BuildNoHashHasher<usize>>,
    /// Next numeric id to use when generating anchor names (1-based).
    next_anchor_id: AnchorId,
    /// If set, the next scalar/complex node to be emitted will be prefixed with this `&anchor`.
    pending_anchor_id: Option<AnchorId>,
    /// Optional custom anchor-name generator supplied by the caller.
    anchor_gen: Option<fn(usize) -> String>,
    /// Cache of custom anchor names when generator is present (index = id-1).
    custom_anchor_names: Option<Vec<String>>,

    // Style flags:
    /// Pending flow-style hint captured from wrapper types.
    pending_flow: Option<PendingFlow>,
    /// Number of nested flow containers we are currently inside (>0 means in-flow).
    in_flow: usize,
    /// Pending block-string style hint (literal `|` or folded `>`).
    pending_str_style: Option<StrStyle>,
    /// Whether the pending block-string style was selected automatically (prefer_block_scalars)
    /// as opposed to being requested explicitly by wrapper types (LitStr/FoldStr variants).
    pending_str_from_auto: bool,
    /// Pending inline comment to be appended after the next scalar (block style only).
    pending_inline_comment: Option<String>,
    /// Placement mode for [`crate::Commented`] wrappers in block style.
    comment_position: CommentPosition,
    /// If true, emit YAML tags for simple enums that serialize to a single scalar.
    tagged_enums: bool,
    /// If true, empty maps are emitted as {} and lists as []
    empty_as_braces: bool,
    /// If true, emit list items with a more compact indentation style under mapping keys.
    compact_list_indent: bool,
    /// If true, automatically prefer YAML block scalars for plain strings:
    ///  - Strings containing newlines use literal style `|`.
    ///  - Single-line strings longer than `folded_wrap_col` use folded style `>`.
    prefer_block_scalars: bool,
    /// When the previous token was a list item dash ("- ") and the next node is a mapping,
    /// emit the first key inline on the same line ("- key: value").
    pending_inline_map: bool,
    /// After writing a mapping key and ':', defer writing the following space until we know
    /// whether the value is a scalar (space) or a complex node (newline with no space).
    pending_space_after_colon: bool,
    /// If the previous sequence element after a dash turned out to be a mapping (inline first key),
    /// indent subsequent dashes by one level to satisfy tests expecting "\n  -".
    inline_map_after_dash: bool,
    /// Whether the last serialized value was a block collection (map or sequence).
    last_value_was_block: bool,
    /// If a sequence element starts with a dash on this depth, capture that depth so
    /// struct-variant mappings emitted immediately after can indent their fields correctly.
    after_dash_depth: Option<usize>,
    /// Current block map indentation depth (for aligning sequences under a map key).
    current_map_depth: Option<usize>,
    /// If true, quote all string scalars. Uses single quotes by default, but switches to
    /// double quotes when the string contains escape sequences or single quotes.
    quote_all: bool,

    /// When enabled, emit YAML 1.2 directive and use YAML 1.2-friendly heuristics.
    yaml_12: bool,
    /// Whether we have started emitting the current document.
    doc_started: bool,
}

impl<'a, W: Write> YamlSerializer<'a, W> {
    /// Construct a `YamlSerializer` that writes to `out`.
    /// Called by `to_writer`/`to_string` entry points.
    pub fn new(out: &'a mut W) -> Self {
        Self {
            out,
            indent_step: 2,
            min_fold_chars: MIN_FOLD_CHARS,
            folded_wrap_col: FOLDED_WRAP_CHARS,
            depth: 0,
            at_line_start: true,
            anchors: HashMap::with_hasher(BuildNoHashHasher::default()),
            next_anchor_id: 1,
            pending_anchor_id: None,
            anchor_gen: None,
            custom_anchor_names: None,
            pending_flow: None,
            in_flow: 0,
            pending_str_style: None,
            pending_str_from_auto: false,
            pending_inline_comment: None,
            comment_position: CommentPosition::Inline,
            tagged_enums: false,
            empty_as_braces: true,
            compact_list_indent: false,
            prefer_block_scalars: true,
            pending_inline_map: false,
            pending_space_after_colon: false,
            inline_map_after_dash: false,
            last_value_was_block: false,
            after_dash_depth: None,
            current_map_depth: None,
            quote_all: false,
            yaml_12: false,
            doc_started: false,
        }
    }
    /// Construct a `YamlSerializer` with a specific indentation step.
    /// Typically used internally by tests or convenience wrappers.
    pub fn with_indent(out: &'a mut W, indent_step: usize) -> Self {
        let mut s = Self::new(out);
        s.indent_step = indent_step;
        s
    }
    /// Construct a `YamlSerializer` from user-supplied [`SerializerOptions`].
    /// Used by `to_writer_with_options`.
    #[allow(deprecated)]
    pub fn with_options(out: &'a mut W, options: &mut SerializerOptions) -> Self {
        let mut s = Self::new(out);
        s.indent_step = options.indent_step;
        s.min_fold_chars = options.min_fold_chars;
        s.folded_wrap_col = options.folded_wrap_chars;
        s.anchor_gen = options.anchor_generator.take();
        s.tagged_enums = options.tagged_enums;
        s.empty_as_braces = options.empty_as_braces;
        s.compact_list_indent = options.compact_list_indent;
        s.prefer_block_scalars = options.prefer_block_scalars;
        s.quote_all = options.quote_all;
        s.comment_position = options.comment_position;
        s.yaml_12 = options.yaml_12;
        s
    }

    // -------- helpers --------

    /// Determines if a string requires double quotes when `quote_all` is enabled.
    /// Returns true if the string contains single quotes, backslashes, or control characters
    /// that need escape processing.
    #[inline]
    fn needs_double_quotes(s: &str) -> bool {
        s.chars().any(|c| {
            c == '\''       // single quote present - cannot use single-quoted style
                || c == '\\' // backslash - needs escape processing
                || is_controll_which_needs_escaping(c) // control chars (includes \n, \t, \r, etc.) need escaping
        })
    }

    /// Write a single-quoted string. Single quotes inside the string are escaped by doubling them.
    fn write_single_quoted(&mut self, s: &str) -> Result<()> {
        self.out.write_char('\'')?;
        for ch in s.chars() {
            if ch == '\'' {
                self.out.write_str("''")?; // escape single quote by doubling
            } else {
                self.out.write_char(ch)?;
            }
        }
        self.out.write_char('\'')?;
        Ok(())
    }

    /// Append a pending inline comment, if any.
    ///
    /// Used both by normal scalar emission (`value # comment\n`) and by
    /// block-scalar headers (`| # comment\n`). Comments are suppressed in flow
    /// style, matching the existing serializer policy.
    #[inline]
    fn write_pending_inline_comment(&mut self) -> Result<()> {
        if self.in_flow == 0
            && let Some(c) = self.pending_inline_comment.take()
        {
            self.out.write_str(" # ")?;
            self.out.write_str(&c)?;
        }
        Ok(())
    }

    /// Called at the end of emitting a scalar in block style: appends a pending inline
    /// comment (if any) and then emits a newline. In flow style, comments are suppressed.
    #[inline]
    fn write_end_of_scalar(&mut self) -> Result<()> {
        if self.in_flow == 0 {
            self.write_pending_inline_comment()?;
            self.newline()?;
        }
        Ok(())
    }

    #[inline]
    fn sanitize_comment_text(comment: &str) -> String {
        comment
            .chars()
            .map(|ch| match ch {
                '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}' => ' ',
                _ => ch,
            })
            .collect()
    }

    fn write_above_comment(&mut self, comment: &str) -> Result<Option<usize>> {
        if self.in_flow > 0 || comment.is_empty() {
            return Ok(None);
        }

        let target_depth = if self.pending_space_after_colon {
            let base = self.current_map_depth.unwrap_or(self.depth);
            self.pending_space_after_colon = false;
            if !self.at_line_start {
                self.newline()?;
            }
            base + 1
        } else if !self.at_line_start {
            let base = self.after_dash_depth.unwrap_or(self.depth);
            self.pending_inline_map = false;
            self.newline()?;
            base + 1
        } else {
            self.depth
        };

        self.write_indent(target_depth)?;
        self.out.write_str("# ")?;
        self.out.write_str(comment)?;
        self.newline()?;
        Ok(Some(target_depth))
    }

    /// Allocate (or get existing) anchor id for a pointer identity.
    /// Returns `(id, is_new)`.
    #[inline]
    fn alloc_anchor_for(&mut self, ptr: usize) -> (AnchorId, bool) {
        match self.anchors.entry(ptr) {
            std::collections::hash_map::Entry::Occupied(e) => (*e.get(), false),
            std::collections::hash_map::Entry::Vacant(v) => {
                let id = self.next_anchor_id;
                self.next_anchor_id = self.next_anchor_id.saturating_add(1);
                if let Some(generator) = self.anchor_gen {
                    let name = generator(id as usize);
                    self.custom_anchor_names
                        .get_or_insert_with(Vec::new)
                        .push(name);
                }
                v.insert(id);
                (id, true)
            }
        }
    }

    /// Resolve an anchor name for `id` and write it.
    #[inline]
    fn write_anchor_name(&mut self, id: AnchorId) -> Result<()> {
        if let Some(names) = &self.custom_anchor_names {
            // ids are 1-based; vec is 0-based
            let idx = id as usize - 1;
            if let Some(name) = names.get(idx) {
                self.out.write_str(name)?;
            } else {
                // Fallback if generator vector is out of sync
                write!(self.out, "a{}", id)?;
            }
        } else {
            write!(self.out, "a{}", id)?;
        }
        Ok(())
    }

    /// If a mapping key has just been written (':' emitted) and we determined the value is a scalar,
    /// insert a single space before the scalar and clear the pending flag.
    #[inline]
    fn write_space_if_pending(&mut self) -> Result<()> {
        if self.pending_space_after_colon {
            self.out.write_char(' ')?;
            self.pending_space_after_colon = false;
        }
        // When a scalar value is serialized, it should reset the block-sibling flag.
        // Most scalar emitters call this method.
        self.last_value_was_block = false;
        Ok(())
    }

    /// Ensure indentation is written if we are at the start of a line.
    /// Internal: called by most emitters before writing tokens.
    #[inline]
    fn write_indent(&mut self, depth: usize) -> Result<()> {
        if self.at_line_start {
            if !self.doc_started {
                self.doc_started = true;
                if self.yaml_12 {
                    self.out.write_str("%YAML 1.2\n")?;
                    // Still at start of a line after the directive.
                    self.at_line_start = true;
                }
            }
            for _k in 0..self.indent_step * depth {
                self.out.write_char(' ')?;
            }
            self.at_line_start = false;
        }
        Ok(())
    }

    /// Emit a newline and mark the next write position as line start.
    /// Internal utility used after finishing a top-level token.
    #[inline]
    fn newline(&mut self) -> Result<()> {
        self.out.write_char('\n')?;
        self.at_line_start = true;
        Ok(())
    }

    /// Write a folded block string body, wrapping to `folded_wrap_col` characters.
    /// Delegates to the standalone function in `wrapping` module.
    fn write_folded_block(&mut self, s: &str, indent: usize) -> Result<()> {
        self::wrapping::write_folded_block(
            self.out,
            s,
            indent,
            self.indent_step,
            self.folded_wrap_col,
        )?;
        self.at_line_start = true;
        Ok(())
    }

    /// Write a scalar either as plain or as double-quoted with minimal escapes.
    /// Called by most `serialize_*` primitive methods.
    fn write_plain_or_quoted(&mut self, s: &str) -> Result<()> {
        if self.quote_all {
            // In quote_all mode: prefer single quotes, use double quotes when needed
            if Self::needs_double_quotes(s) {
                self.write_quoted(s)
            } else {
                self.write_single_quoted(s)
            }
        } else if is_plain_safe(s) {
            self.out.write_str(s)?;
            Ok(())
        } else {
            self.write_quoted(s)
        }
    }

    /// Write a double-quoted string with necessary escapes.
    fn write_quoted(&mut self, s: &str) -> Result<()> {
        self.out.write_char('"')?;
        for ch in s.chars() {
            match ch {
                '\\' => self.out.write_str("\\\\")?,
                '"' => self.out.write_str("\\\"")?,
                // YAML named escapes for common control characters
                '\0' => self.out.write_str("\\0")?,
                '\u{7}' => self.out.write_str("\\a")?,
                '\u{8}' => self.out.write_str("\\b")?,
                '\t' => self.out.write_str("\\t")?,
                '\n' => self.out.write_str("\\n")?,
                '\u{b}' => self.out.write_str("\\v")?,
                '\u{c}' => self.out.write_str("\\f")?,
                '\r' => self.out.write_str("\\r")?,
                '\u{1b}' => self.out.write_str("\\e")?,
                // Unicode BOM should use the standard \u escape rather than Rust's \u{...}
                '\u{FEFF}' => self.out.write_str("\\uFEFF")?,
                // YAML named escapes for Unicode separators
                '\u{0085}' => self.out.write_str("\\N")?,
                '\u{2028}' => self.out.write_str("\\L")?,
                '\u{2029}' => self.out.write_str("\\P")?,
                c if (c as u32) <= 0xFF
                    && (c.is_control() || (0x7F..=0x9F).contains(&(c as u32))) =>
                {
                    write!(self.out, "\\x{:02X}", c as u32)?
                }
                c if (c as u32) <= 0xFFFF
                    && (c.is_control() || (0x7F..=0x9F).contains(&(c as u32))) =>
                {
                    write!(self.out, "\\u{:04X}", c as u32)?
                }
                c => self.out.write_char(c)?,
            }
        }
        self.out.write_char('"')?;
        Ok(())
    }

    /// Like `write_plain_or_quoted`, but intended for VALUE position where ':' is allowed.
    #[inline]
    fn write_plain_or_quoted_value(&mut self, s: &str) -> Result<()> {
        if self.quote_all {
            // In quote_all mode: prefer single quotes, use double quotes when needed
            if Self::needs_double_quotes(s) {
                self.write_quoted(s)
            } else {
                self.write_single_quoted(s)
            }
        } else if is_plain_value_safe(s, self.yaml_12, self.in_flow > 0) {
            self.out.write_str(s)?;
            Ok(())
        } else {
            // Force quoted style for problematic value tokens (commas/brackets, bool/num-like, etc.).
            self.write_quoted(s)
        }
    }

    /// Serialize a tagged scalar of the form `!!Type value` using plain or quoted style for
    /// the value depending on its content.
    fn serialize_tagged_scalar(&mut self, enum_name: &str, variant: &str) -> Result<()> {
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.out.write_str("!!")?;
        self.out.write_str(enum_name)?;
        self.out.write_char(' ')?;
        self.write_plain_or_quoted_value(variant)?;
        self.write_end_of_scalar()
    }

    fn serialize_double_quoted_scalar(&mut self, value: &str) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_quoted(value)?;
        self.write_end_of_scalar()
    }

    fn serialize_single_quoted_scalar(&mut self, value: &str) -> Result<()> {
        if let Some(ch) = value
            .chars()
            .find(|ch| is_controll_which_needs_escaping(*ch))
        {
            return Err(Error::SingleQuotedRequiresEscaping { ch });
        }
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_single_quoted(value)?;
        self.write_end_of_scalar()
    }

    fn serialize_tilde_null(&mut self) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.out.write_char('~')?;
        self.write_end_of_scalar()
    }

    /// If an anchor is pending for the next scalar, emit `&name ` prefix.
    /// Used for in-flow scalars.
    #[inline]
    fn write_scalar_prefix_if_anchor(&mut self) -> Result<()> {
        if let Some(id) = self.pending_anchor_id.take() {
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            self.out.write_char('&')?;
            self.write_anchor_name(id)?;
            self.out.write_char(' ')?;
        }
        Ok(())
    }

    /// If an anchor is pending for the next complex node (seq/map),
    /// emit it on its own line before the node.
    #[inline]
    fn write_anchor_for_complex_node(&mut self) -> Result<()> {
        if let Some(id) = self.pending_anchor_id.take() {
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            self.write_space_if_pending()?;
            self.out.write_char('&')?;
            self.write_anchor_name(id)?;
            self.newline()?;
        }
        Ok(())
    }

    /// Emit an alias `*name`. Adds a newline in block style.
    /// Used when a previously defined anchor is referenced again.
    #[inline]
    fn write_alias_id(&mut self, id: AnchorId) -> Result<()> {
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_space_if_pending()?;
        self.out.write_char('*')?;
        self.write_anchor_name(id)?;
        // Use the shared end-of-scalar path so pending inline comments are appended in block style
        self.write_end_of_scalar()?;
        Ok(())
    }

    /// Determine whether the next sequence should be emitted in flow style.
    /// Consumes any pending flow hint.
    #[inline]
    fn take_flow_for_seq(&mut self) -> bool {
        if self.in_flow > 0 {
            true
        } else {
            matches!(self.pending_flow.take(), Some(PendingFlow::AnySeq))
        }
    }
    /// Determine whether the next mapping should be emitted in flow style.
    /// Consumes any pending flow hint.
    #[inline]
    fn take_flow_for_map(&mut self) -> bool {
        if self.in_flow > 0 {
            true
        } else {
            matches!(self.pending_flow.take(), Some(PendingFlow::AnyMap))
        }
    }

    /// Temporarily mark that we are inside a flow container while running `f`.
    /// Ensures proper comma insertion and line handling for nested flow nodes.
    #[inline]
    fn with_in_flow<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.in_flow += 1;
        let r = f(self);
        self.in_flow -= 1;
        r
    }
}

// ------------------------------------------------------------
// Impl Serializer for YamlSerializer
// ------------------------------------------------------------

impl<'a, 'b, W: Write> Serializer for &'a mut YamlSerializer<'b, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSer<'a, 'b, W>;
    type SerializeTuple = SeqSer<'a, 'b, W>;
    type SerializeTupleStruct = TupleSer<'a, 'b, W>;
    type SerializeTupleVariant = SeqSer<'a, 'b, W>;
    type SerializeMap = MapSer<'a, 'b, W>;
    type SerializeStruct = MapSer<'a, 'b, W>;
    type SerializeStructVariant = StructVariantSer<'a, 'b, W>;

    // -------- Scalars --------

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.out.write_str(if v { "true" } else { "false" })?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        write!(self.out, "{}", v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        write!(self.out, "{}", v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        write!(self.out, "{}", v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        write!(self.out, "{}", v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        zmij_format::write_float_string(self.out, v)?;
        self.write_end_of_scalar()
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        zmij_format::write_float_string(self.out, v)?;
        self.write_end_of_scalar()
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.write_space_if_pending()?;
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        #[inline]
        fn block_indent_indicator_digit(indent_n: usize) -> Result<char> {
            // YAML 1.2 8.1.1.1: the block-scalar indentation indicator is `[1-9]`.
            // `char::from_digit` would accept 0, so we gate on the range explicitly.
            match indent_n {
                1..=9 => {
                    let digit = u32::try_from(indent_n).expect("checked 1..=9");
                    Ok(char::from_digit(digit, 10).expect("checked 1..=9"))
                }
                _ => Err(Error::custom(
                    "indentation indicator must be a single digit (1..=9)",
                )),
            }
        }

        // If no explicit style pending, auto-select block style.
        //
        // Controlled by `prefer_block_scalars`:
        //  - multiline → literal (|) whenever the content is representable in a block scalar
        //    and is readable enough to auto-select (see `is_auto_block_scalar_readable`).
        //    Block scalars happily contain ':', '#', YAML-like text, etc. — those are unsafe
        //    in plain style but fine as block content.
        //  - single-line + long (by folded_wrap_col) → folded (>)
        //
        // Also skip block scalars when quote_all is enabled - use quoted strings instead.
        if self.pending_str_style.is_none() && self.in_flow == 0 && !self.quote_all {
            if v.contains('\n') {
                if self.prefer_block_scalars
                    && is_block_scalar_content_safe(v)
                    && is_auto_block_scalar_readable(v)
                {
                    self.pending_str_style = Some(StrStyle::Literal);
                    self.pending_str_from_auto = true;
                }
            } else if self.prefer_block_scalars {
                // Single-line string. If it needs quoting as a value, don't auto-fold.
                let needs_quoting = !is_plain_value_safe(v, self.yaml_12, false);
                if !needs_quoting {
                    // Measure in characters, not bytes.
                    if v.chars().count() > self.folded_wrap_col {
                        self.pending_str_style = Some(StrStyle::Folded);
                        self.pending_str_from_auto = true;
                    }
                }
            }
        }
        if let Some(style) = self.pending_str_style.take() {
            if !is_block_scalar_content_safe(v) {
                self.pending_str_from_auto = false;
                self.write_space_if_pending()?;
                self.write_scalar_prefix_if_anchor()?;
                if self.at_line_start {
                    self.write_indent(self.depth)?;
                }
                self.write_quoted(v)?;
                self.write_end_of_scalar()?;
                return Ok(());
            }

            // Emit block string. If we are a mapping value, YAML requires a space after ':'.
            // Insert it now if pending.
            //
            // IMPORTANT: capture whether we were in a map-value position *before* clearing
            // `pending_space_after_colon`, as that context influences indentation.
            let was_map_value = self.pending_space_after_colon;
            self.write_space_if_pending()?;
            // Determine base indentation for the block scalar header/body.
            //
            // Important: `after_dash_depth` is only meaningful for the immediate node that
            // follows a sequence dash ("- "). It must NOT affect nested scalars inside a
            // mapping that happens to be a sequence element, otherwise block scalar bodies
            // become under-indented (invalid YAML).
            //
            // For map values (we are mid-line after `key:`), prefer the mapping depth.
            // Otherwise, if we are starting a new node right after a dash, use that depth.
            let base = if was_map_value {
                self.current_map_depth.unwrap_or(self.depth)
            } else {
                // Use after_dash_depth when available (we're after a sequence dash),
                // regardless of at_line_start (which is false after writing "- ").
                self.after_dash_depth.unwrap_or(self.depth)
            };
            if self.at_line_start {
                self.write_indent(base)?;
            }

            // Anchors/tags are node properties and must be emitted before scalar content.
            // For a block scalar this produces e.g. `text: &a1 |` — without this, a pending
            // anchor would leak onto the next scalar.
            self.write_scalar_prefix_if_anchor()?;

            // Compute the indentation indicator N for block scalars.
            //
            // Per YAML 1.2 8.1.1.1, the indicator is the *additional* indentation steps
            // beyond the parent node's indentation level — NOT the absolute column count.
            // Since our body is exactly one serializer depth (i.e. `indent_step` spaces)
            // deeper than its parent, the indicator is simply `indent_step`.
            //
            // We only emit it when the first non-empty content line has leading whitespace,
            // which would otherwise prevent automatic indentation detection by the parser.
            let body_base = base + 1;
            let indent_n = self.indent_step;

            // Check if we need an explicit indentation indicator.
            // Required when the first non-empty line has leading whitespace.
            let content_trimmed = v.trim_end_matches('\n');
            let first_line_spaces = self::wrapping::first_line_leading_spaces(content_trimmed);
            let needs_indicator = first_line_spaces > 0;

            // Resolve the indicator digit up front. If the helper rejects `indent_n`
            // (i.e. outside the YAML 1.2 `[1-9]` grammar), fall back to quoting.
            // Anchor prefix is already written above, so don't call it again here.
            let indicator_digit = if needs_indicator {
                let Ok(digit) = block_indent_indicator_digit(indent_n) else {
                    self.pending_str_style = None;
                    self.pending_str_from_auto = false;
                    self.write_plain_or_quoted_value(v)?;
                    self.write_end_of_scalar()?;
                    return Ok(());
                };
                Some(digit)
            } else {
                None
            };

            match style {
                StrStyle::Literal => {
                    // Determine trailing newline count to select chomp indicator:
                    //  - 0 → "|-" (strip)
                    //  - 1 → "|" (clip)
                    //  - >=2 → "|+" (keep)
                    let content = v.trim_end_matches('\n');
                    let trailing_nl = v.len() - content.len();

                    // Write block scalar header: | or |N with optional chomp indicator
                    self.out.write_char('|')?;
                    if let Some(digit) = indicator_digit {
                        self.out.write_char(digit)?;
                    }
                    match trailing_nl {
                        0 => self.out.write_char('-')?,
                        1 => {} // clip is the default, no indicator needed
                        _ => self.out.write_char('+')?,
                    }
                    self.write_pending_inline_comment()?;
                    self.newline()?;

                    // Emit body lines. For non-empty content, write each line exactly once.
                    // For keep chomping (>=2), append (trailing_nl - 1) visual empty lines.
                    // Special case: empty original content with at least one trailing newline
                    // should produce a single empty content line (tests expect this for "\n").
                    // Precompute body indent string once for the entire block
                    let mut indent_buf: String = String::new();
                    let spaces = self.indent_step * body_base;
                    if spaces > 0 {
                        indent_buf.reserve(spaces);
                        for _ in 0..spaces {
                            indent_buf.push(' ');
                        }
                    }
                    let indent_str = indent_buf.as_str();

                    if content.is_empty() {
                        if trailing_nl >= 1 {
                            self.out.write_str(indent_str)?;
                            self.at_line_start = false;
                            // write a single empty content line
                            self.newline()?;
                        }
                    } else {
                        for line in content.split('\n') {
                            self.out.write_str(indent_str)?;
                            self.at_line_start = false;
                            self.out.write_str(line)?;
                            self.newline()?;
                        }
                        if trailing_nl >= 2 {
                            for _ in 0..(trailing_nl - 1) {
                                self.out.write_str(indent_str)?;
                                self.at_line_start = false;
                                self.newline()?;
                            }
                        }
                    }
                }
                StrStyle::Folded => {
                    // Write block scalar header: > or >N with optional chomp indicator
                    self.out.write_char('>')?;
                    if let Some(digit) = indicator_digit {
                        self.out.write_char(digit)?;
                    }
                    if self.pending_str_from_auto {
                        // Auto-selected folded style: choose chomping based on trailing newlines
                        // to preserve exact content on round-trip.
                        let content = v.trim_end_matches('\n');
                        let trailing_nl = v.len() - content.len();
                        match trailing_nl {
                            0 => self.out.write_char('-')?,
                            1 => {} // clip is the default, no indicator needed
                            _ => self.out.write_char('+')?,
                        }
                    }
                    // Note: Explicit FoldStr/FoldString wrappers historically used plain '>'
                    // regardless of trailing newline; keep that behavior for compatibility.
                    self.write_pending_inline_comment()?;
                    self.newline()?;
                    self.write_folded_block(v, body_base)?;
                }
            }
            // reset auto flag after using pending style
            self.pending_str_from_auto = false;
            return Ok(());
        }
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        // Special-case: prefer single-quoted style for select 1-char punctuation to
        // match expected YAML output in tests ('.', '#', '-').
        if v.len() == 1
            && let Some(ch) = v.chars().next()
            && (ch == '.' || ch == '#' || ch == '-')
        {
            self.out.write_char('\'')?;
            self.out.write_char(ch)?;
            self.out.write_char('\'')?;
            self.write_end_of_scalar()?;
            return Ok(());
        }
        self.write_plain_or_quoted_value(v)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        // Two behaviors are required by tests:
        // - Top-level &[u8] should serialize as a block sequence of integers.
        // - Fields using #[serde(with = "serde_bytes")] should serialize as a tagged !!binary
        //   base64 scalar inline after "key: ". The latter ends up calling serialize_bytes in
        //   value position (mid-line), whereas plain Vec<u8> without serde_bytes goes through
        //   serialize_seq instead. Distinguish by whether we are at the start of a line.
        if self.at_line_start {
            // Top-level or start-of-line: emit as sequence of numbers
            let mut seq = self.serialize_seq(Some(v.len()))?;
            for b in v {
                serde_core::ser::SerializeSeq::serialize_element(&mut seq, b)?;
            }
            return serde_core::ser::SerializeSeq::end(seq);
        }

        // Inline value position: emit !!binary with base64.
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        // No indent needed mid-line; mirror serialize_str behavior.
        self.out.write_str("!!binary ")?;
        let mut s = String::new();
        B64.encode_string(v, &mut s);
        self.out.write_str(&s)?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.write_space_if_pending()?;
        self.last_value_was_block = false;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.out.write_str("null")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.write_space_if_pending()?;
        self.last_value_was_block = false;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.out.write_str("null")?;
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        // If we are in a mapping value position, insert the deferred space after ':'
        self.write_space_if_pending()?;
        if self.tagged_enums {
            self.serialize_tagged_scalar(name, variant)
        } else {
            self.serialize_str(variant)
        }
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        // Flow hints & block-string hints:
        match name {
            NAME_FLOW_SEQ => {
                self.pending_flow = Some(PendingFlow::AnySeq);
                return value.serialize(self);
            }
            NAME_FLOW_MAP => {
                self.pending_flow = Some(PendingFlow::AnyMap);
                return value.serialize(self);
            }
            NAME_LIT_STR => {
                // Always use literal block style for LitStr/LitString wrappers.
                // Choose chomping based on trailing newlines during actual emission.
                // Capture the inner string first.
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let s = cap.finish()?;
                self.pending_str_style = Some(StrStyle::Literal);
                return self.serialize_str(&s);
            }
            NAME_FOLD_STR => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let s = cap.finish()?;
                let is_multiline = s.contains('\n');
                if !is_multiline && s.len() < self.min_fold_chars {
                    return self.serialize_str(&s);
                }
                self.pending_str_style = Some(StrStyle::Folded);
                return self.serialize_str(&s);
            }
            NAME_SPACE_AFTER => {
                // Serialize the value, then emit an empty line after (only in block style).
                let result = value.serialize(&mut *self);
                if self.in_flow == 0 {
                    // Emit an extra blank line after the value
                    self.newline()?;
                }
                return result;
            }
            NAME_DOUBLE_QUOTED => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let text = cap.finish()?;
                return self.serialize_double_quoted_scalar(&text);
            }
            NAME_SINGLE_QUOTED => {
                let mut cap = StrCapture::default();
                value.serialize(&mut cap)?;
                let text = cap.finish()?;
                return self.serialize_single_quoted_scalar(&text);
            }
            NAME_NULLABLE_TILDE => {
                return self.serialize_tilde_null();
            }
            _ => {}
        }
        // default: ignore the name, serialize the inner as-is
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        // If we are the value of a mapping key, YAML forbids "key: Variant: value" inline.
        // Emit the variant mapping on the next line indented one level. Also, do not insert
        // a space after the colon when the value may itself be a mapping; instead, defer
        // space insertion to the value serializer via pending_space_after_colon.
        if self.pending_space_after_colon {
            // consume the pending space request and start a new line
            self.pending_space_after_colon = false;
            self.newline()?;
            // When used as a mapping value, indent relative to the parent mapping's base,
            // not the serializer's current depth (which may still be the outer level).
            let base = self.current_map_depth.unwrap_or(self.depth);
            self.write_indent(base + 1)?;
            self.write_plain_or_quoted(variant)?;
            // Write ':' without trailing space, then mark that a space may be needed
            // if the following value is a scalar.
            self.out.write_str(":")?;
            self.pending_space_after_colon = true;
            self.at_line_start = false;
            // Do not let any inline-after-dash hint leak into the variant's inner value.
            // After `Variant:`, the next node is in value position and must choose its own layout.
            self.pending_inline_map = false;
            // Ensure that if the value is another variant or a mapping/sequence,
            // it indents under this variant label rather than the parent map key.
            let prev_map_depth = self.current_map_depth.replace(base + 1);
            let res = value.serialize(&mut *self);
            self.current_map_depth = prev_map_depth;
            return res;
        }
        // Otherwise (top-level or sequence context).
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_plain_or_quoted(variant)?;
        // Write ':' without a space and defer spacing/newline to the value serializer.
        self.out.write_str(":")?;
        self.pending_space_after_colon = true;
        self.at_line_start = false;
        // Do not let SeqSer's "inline first key after dash" hint leak into the variant's inner value.
        // Without this, a struct/map value can start as `Variant: a: 1`.
        self.pending_inline_map = false;
        // If this variant is inside a block sequence element (`- Variant:`), ensure the nested
        // value indents under the variant label rather than aligning with the list indentation.
        // SeqSer stores the dash's indentation depth in `after_dash_depth`.
        if let Some(d) = self.after_dash_depth.take() {
            let prev_map_depth = self.current_map_depth.replace(d + 1);
            let res = value.serialize(&mut *self);
            self.current_map_depth = prev_map_depth;
            res
        } else {
            value.serialize(&mut *self)
        }
    }

    // -------- Collections --------

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        let flow = self.take_flow_for_seq();
        if flow {
            self.write_scalar_prefix_if_anchor()?;
            // Ensure a space after a preceding colon when this sequence is a mapping value.
            self.write_space_if_pending()?;
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            self.out.write_str("[")?;
            self.at_line_start = false;
            let depth_next = self.depth; // inline
            Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: true,
                first: true,
            })
        } else {
            // Block sequence. Decide indentation based on whether this is after a map key or after a list dash.
            let was_inline_value = !self.at_line_start;

            // If we are a value following a block sibling, force a newline now.
            // However, if a complex-node anchor is pending, we must keep `key: &aN` inline;
            // `write_anchor_for_complex_node` will handle emitting the anchor and newline.
            if self.pending_space_after_colon
                && self.last_value_was_block
                && self.pending_anchor_id.is_none()
            {
                self.pending_space_after_colon = false;
                if !self.at_line_start {
                    self.newline()?;
                }
                // Consume the sibling-block marker; it should not affect nested nodes.
                self.last_value_was_block = false;
            }

            // For block sequences nested under another dash, keep the first inner dash inline.
            // Style expectations in tests prefer the compact form:
            // - - 1
            // instead of:
            // -
            //   - 1
            let inline_first = (!self.at_line_start)
                && self.after_dash_depth.is_some()
                && !self.pending_space_after_colon;
            // `inline_first` assumes we stay mid-line, but a pending anchor writes `&aN\n` first.
            let anchor_broke_line = self.pending_anchor_id.is_some();
            self.write_anchor_for_complex_node()?;
            if inline_first {
                if anchor_broke_line {
                    // Inlining now would drop the nested dashes to column 0, past the anchor.
                    self.pending_inline_map = false;
                } else {
                    // Collapsing onto the parent dash yields the preferred `- - 1` shape.
                    self.at_line_start = false;
                }
            } else if was_inline_value {
                // Mid-line start. If we are here due to a map value (after ':'), defer the newline
                // decision until the first element is emitted so that empty sequences can stay inline
                // as `key: []`. If we are here due to a list dash, keep inline.
                // Intentionally do not clear `pending_space_after_colon` and do not newline here.
            }
            // Indentation policy mirrors serialize_map:
            // - After a list dash inline_first: base is dash depth; indent one level deeper.
            // - As a value after a map key: base is current_map_depth (if set), indent one level deeper.
            // - Otherwise (top-level or already at line start): base is current depth.
            let base = if inline_first {
                self.after_dash_depth.unwrap_or(self.depth)
            } else if was_inline_value && self.current_map_depth.is_some() {
                self.current_map_depth.unwrap_or(self.depth)
            } else {
                self.depth
            };
            // For sequences used as a mapping value, indent them one level deeper so the dash is
            // nested under the parent key (consistent with serde_yaml's formatting). Keep block
            // sequences inline only when they immediately follow another dash.
            let depth_next = if inline_first {
                base + 1
            } else if was_inline_value {
                if self.compact_list_indent && self.current_map_depth.is_some() {
                    base
                } else {
                    base + 1
                }
            } else {
                base
            };
            // Starting a complex (block) sequence: drop any staged inline comment.
            self.pending_inline_comment = None;
            Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
            })
        }
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        if name == NAME_TUPLE_ANCHOR {
            Ok(TupleSer::anchor_strong(self))
        } else if name == NAME_TUPLE_WEAK {
            Ok(TupleSer::anchor_weak(self))
        } else if name == NAME_TUPLE_COMMENTED {
            Ok(TupleSer::commented(self))
        } else {
            // Normal tuple-struct: emit as a block sequence.
            Ok(TupleSer::Seq(self.serialize_seq(Some(len))?))
        }
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        // If we are the value of a mapping key, YAML forbids keeping a nested mapping
        // on the same line (e.g., "key: Variant:"). Move the variant mapping to the next line
        // indented under the parent mapping's base depth.
        if self.pending_space_after_colon {
            self.pending_space_after_colon = false;
            self.newline()?;
            let base = self.current_map_depth.unwrap_or(self.depth) + 1;
            self.write_indent(base)?;
            self.write_plain_or_quoted(variant)?;
            self.out.write_str(":\n")?;
            self.at_line_start = true;
            self.pending_inline_map = false;
            let depth_next = base + 1;
            return Ok(SeqSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
            });
        }
        // Otherwise (top-level or sequence context).
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(":\n")?;
        self.at_line_start = true;
        let mut depth_next = self.depth + 1;
        if let Some(d) = self.after_dash_depth.take() {
            depth_next = d + 2;
            self.pending_inline_map = false;
        }
        Ok(SeqSer {
            ser: self,
            depth: depth_next,
            flow: false,
            first: true,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let flow = self.take_flow_for_map();
        if flow {
            self.write_scalar_prefix_if_anchor()?;
            // Ensure a space after a preceding colon when this mapping is a value.
            self.write_space_if_pending()?;
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            self.out.write_str("{")?;
            self.at_line_start = false;
            let depth_next = self.depth;
            Ok(MapSer {
                ser: self,
                depth: depth_next,
                flow: true,
                first: true,
                last_key_complex: false,
                align_after_dash: false,
                inline_value_start: false,
            })
        } else {
            let inline_first = self.pending_inline_map;
            // We only consider "value position" when immediately after a mapping colon.
            let was_inline_value = self.pending_space_after_colon;
            let mut forced_newline = false;

            // If we are a value following a block sibling, force a newline now.
            // However, if a complex-node anchor is pending, we must keep `key: &aN` inline;
            // `write_anchor_for_complex_node` will handle emitting the anchor and newline.
            if was_inline_value && self.last_value_was_block && self.pending_anchor_id.is_none() {
                self.pending_space_after_colon = false;
                if !self.at_line_start {
                    self.newline()?;
                }
                forced_newline = true;
                // Consume the sibling-block marker; it should not affect nested nodes.
                self.last_value_was_block = false;
            }

            self.write_anchor_for_complex_node()?;
            if inline_first {
                // Suppress newline after a list dash for inline map first key.
                self.pending_inline_map = false;
                // Mark that this sequence element is a mapping printed inline after a dash.
                self.inline_map_after_dash = true;
            } else if was_inline_value {
                // Map used as a value after "key: ". If emitting braces for empty maps,
                // keep this mapping on the same line so that an empty map renders as "{}".
                //
                // IMPORTANT: if the map is known to be non-empty (len > 0), we must NOT keep it
                // inline (otherwise we can end up emitting the first entry as `key: a: 1`).
                // When len is unknown, we keep the legacy behavior and let MapSer decide once the
                // first key arrives.
                let known_empty = matches!(len, Some(0));
                let known_non_empty = matches!(len, Some(n) if n > 0);

                if !self.empty_as_braces || known_non_empty {
                    // Move the mapping body to the next line.
                    // If an anchor was emitted, we are already at the start of a new line.
                    self.pending_space_after_colon = false;
                    if !self.at_line_start {
                        self.newline()?;
                    }
                } else if !known_empty {
                    // len is unknown: keep it inline for now (so empty maps can still render as
                    // `key: {}`), and let MapSer break the line when the first key arrives.
                }
            }
            // Indentation rules:
            // - Top-level (at line start, not after dash): use current depth.
            // - After dash inline first key or as a value: indent one level deeper for subsequent lines.
            // Use the current mapping's depth as base only when we are in a VALUE position.
            // For complex KEYS (non-scalar), keep using the current serializer depth so that
            // subsequent key lines indent relative to the "? " line, not the parent map's base.
            let base = if inline_first {
                self.after_dash_depth.unwrap_or(self.depth)
            } else if was_inline_value && self.current_map_depth.is_some() {
                self.current_map_depth.unwrap_or(self.depth)
            } else {
                self.depth
            };
            let depth_next = if inline_first || was_inline_value {
                base + 1
            } else {
                base
            };
            let inline_value_start_flag = was_inline_value
                && self.empty_as_braces
                && len.is_none()
                && !inline_first
                && !forced_newline;
            Ok(MapSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
                last_key_complex: false,
                align_after_dash: inline_first,
                inline_value_start: inline_value_start_flag,
            })
        }
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        // If we are the value of a mapping key, YAML forbids keeping a nested mapping
        // on the same line (e.g., "key: Variant:"). Move the variant mapping to the next line
        // indented under the parent mapping's base depth.
        if self.pending_space_after_colon {
            // Value position after a map key: start the variant mapping on the next line.
            self.pending_space_after_colon = false;
            self.newline()?;
            // Indent the variant name one level under the parent mapping.
            let base = self.current_map_depth.unwrap_or(self.depth) + 1;
            self.write_indent(base)?;
            self.write_plain_or_quoted(variant)?;
            self.out.write_str(":\n")?;
            self.at_line_start = true;
            // A complex key stages an inline hint for its value; clear it before the fields.
            self.pending_inline_map = false;
            // Fields indent one more level under the variant label.
            let depth_next = base + 1;
            return Ok(StructVariantSer {
                ser: self,
                depth: depth_next,
            });
        }
        // Otherwise (top-level or sequence context), emit the variant name at current depth.
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(":\n")?;
        self.at_line_start = true;
        // Default indentation for fields under a plain variant line.
        let mut depth_next = self.depth + 1;
        // If this variant follows a list dash, indent two levels under the dash (one for the element, one for the mapping).
        if let Some(d) = self.after_dash_depth.take() {
            depth_next = d + 2;
            self.pending_inline_map = false;
        }
        Ok(StructVariantSer {
            ser: self,
            depth: depth_next,
        })
    }
}
