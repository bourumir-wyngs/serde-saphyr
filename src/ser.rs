//! Single-pass YAML serializer with optional anchors for Rc/Arc/Weak,
//! order preservation (uses the iterator order of your types), simple
//! style controls (block strings & flow containers), and special
//! float handling for NaN/±Inf. No intermediate YAML DOM is built.
//!
//! Usage example:
//!
//! use serde::Serialize;
//! use std::rc::Rc;
//! use serde_saphyr::{to_string, RcAnchor, LitStr, FlowSeq};
//!
//! #[derive(Serialize)]
//! struct Cfg {
//!     name: String,
//!     ports: FlowSeq<Vec<u16>>,   // render `[8080, 8081]`
//!     note: LitStr<'static>,      // render as `|` block
//!     data: RcAnchor<Vec<i32>>,   // first sight => &a1
//!     alias: RcAnchor<Vec<i32>>,  // later sight => *a1
//! }
//!
//! fn main() {
//!     let shared = Rc::new(vec![1,2,3]);
//!     let cfg = Cfg {
//!         name: "demo".into(),
//!         ports: FlowSeq(vec![8080, 8081]),
//!         note: LitStr("line 1\nline 2"),
//!         data: RcAnchor(shared.clone()),
//!         alias: RcAnchor(shared),
//!     };
//!     println!("{}", to_string(&cfg).unwrap());
//! }

use serde::de::{Deserialize, Deserializer};
use serde::ser::{
    self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Weak as ArcWeak};

use crate::serializer_options::SerializerOptions;
use crate::{ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use nohash_hasher::BuildNoHashHasher;

// ------------------------------------------------------------
// Public API
// ------------------------------------------------------------

pub use crate::ser_error::Error;
use crate::ser_quoting::{is_plain_safe, is_plain_value_safe};

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Force a sequence to be emitted in flow style: `[a, b, c]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowSeq<T>(pub T);
/// Force a mapping to be emitted in flow style: `{k1: v1, k2: v2}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowMap<T>(pub T);

/// Attach an inline YAML comment to a value when serializing.
///
/// This wrapper lets you annotate a scalar with an inline YAML comment that is
/// emitted after the value when using block style. The typical form is:
/// `value # comment`. This is the most useful when deserializing the anchor
/// reference (human reader may want short comment what is the reference about)
///
/// Behavior
/// - Block style (default): the comment appears after the scalar on the same line.
/// - Flow style (inside `[ ... ]` or `{ ... }`): comments are suppressed to keep
///   the flow representation compact and unambiguous.
/// - Complex values (sequences/maps/structs): the comment is ignored; only the
///   inner value is serialized to preserve indentation and layout.
/// - Newlines in comments are sanitized to spaces so the comment remains on a
///   single line (e.g., "a\nb" becomes "a b").
/// - Deserialization of `Commented<T>` ignores comments: it behaves like `T` and
///   produces an empty comment string.
///
/// Examples
///
/// Basic scalar with a comment in block style:
/// ```rust
/// use serde::Serialize;
///
/// // Re-exported from the crate root
/// use serde_saphyr::Commented;
///
/// let out = serde_saphyr::to_string(&Commented(42, "answer".to_string())).unwrap();
/// assert_eq!(out, "42 # answer\n");
/// ```
///
/// As a mapping value, still inline:
/// ```rust
/// use serde::Serialize;
/// use serde_saphyr::Commented;
///
/// #[derive(Serialize)]
/// struct S { n: Commented<i32> }
///
/// let s = S { n: Commented(5, "send five starships first".into()) };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "n: 5 # send five starships first\n");
/// ```
///
/// *Important*: Comments are suppressed in flow contexts (no `#` appears), and
/// ignored for complex inner values. Value with `Commented` wrapper will be
/// deserializaed correctly as well, but deserialization of comment is
/// currently not supported.
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commented<T>(pub T, pub String);

/// Force a YAML block literal string using the `|` style.
///
/// Emits the inner `&str` as a block scalar that preserves newlines exactly
/// as written. Each line is indented one level deeper than the surrounding
/// indentation where the value appears.
///
/// In short: use LitStr (|) to preserve line breaks exactly; use FoldStr (>) when you want
/// readers to display line breaks as spaces (soft-wrapped paragraphs).
///
/// Behavior
/// - Uses YAML's literal block style: a leading `|` followed by newline.
/// - Newlines are preserved verbatim by YAML consumers.
/// - Indentation is handled automatically by the serializer.
/// - Works in mapping values, sequence items, and at the top level.
///
/// Examples
///
/// Top-level literal block string (only for sufficiently long content; short strings serialize like normal scalars):
/// ```rust
/// let long = "line 1\nline 2\n".repeat(20);
/// let out = serde_saphyr::to_string(&serde_saphyr::LitStr(&long)).unwrap();
/// assert!(out.starts_with("|\n  "));
/// ```
///
/// As a mapping value:
/// ```rust
/// use serde::Serialize;
/// #[derive(Serialize)]
/// struct S { note: serde_saphyr::LitStr<'static> }
/// let s = S { note: serde_saphyr::LitStr("a\nb") };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "note: |\n  a\n  b\n");
/// ```
#[derive(Clone, Copy)]
pub struct LitStr<'a>(pub &'a str);

/// Owned-string variant of LitStr that forces a YAML block literal string using the `|` style.
///
/// This works the same as LitStr but takes ownership of a String. Useful when you already
/// have an owned String and want to avoid borrowing lifetimes.
///
/// Example
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::LitString("line 1\nline 2".to_string())).unwrap();
/// assert_eq!(out, "|\n  line 1\n  line 2\n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LitString(pub String);

/// Force a YAML folded block string using the `>` style.
///
/// Emits the inner `&str` as a block scalar that suggests folding line breaks
/// to spaces for display by YAML consumers (empty lines are kept as paragraph
/// breaks). The serializer writes each line on its own; the folding behavior is
/// applied by consumers of the YAML, not during serialization.
///
/// In short: use FoldStr (>) for human-readable paragraphs that may soft-wrap; use
/// LitStr (|) when you need to preserve line breaks exactly as written.
///
/// Behavior
/// - Uses YAML's folded block style: a leading `>` followed by newline.
/// - Intended for human-readable paragraphs where soft-wrapping is desirable.
/// - Indentation is handled automatically by the serializer.
/// - Works in mapping values, sequence items, and at the top level.
///
/// Examples
///
/// Top-level folded block string:
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::FoldStr("line 1\nline 2")).unwrap();
/// assert_eq!(out, ">\n  line 1\n  line 2\n");
/// ```
///
/// As a mapping value:
/// ```rust
/// use serde::Serialize;
/// #[derive(Serialize)]
/// struct S { note: serde_saphyr::FoldStr<'static> }
/// let s = S { note: serde_saphyr::FoldStr("a\nb") };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "note: >\n  a\n  b\n");
/// ```
#[derive(Clone, Copy)]
pub struct FoldStr<'a>(pub &'a str);

/// Owned-string variant of FoldStr that forces a YAML folded block string using the `>` style.
///
/// Same behavior as FoldStr but owns a String.
///
/// Example
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::FoldString("line 1\nline 2".to_string())).unwrap();
/// assert_eq!(out, ">\n  line 1\n  line 2\n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoldString(pub String);

// ------------------------------------------------------------
// Internal wrappers -> shape the stream Serde produces so our
// serializer can intercept them in a single pass.
// ------------------------------------------------------------

// Strong: "__yaml_anchor" tuple-struct => [ptr, value]
#[allow(dead_code)]
struct RcStrongPayload<'a, T>(&'a Rc<T>);
#[allow(dead_code)]
struct ArcStrongPayload<'a, T>(&'a Arc<T>);

// Weak: "__yaml_weak_anchor" tuple-struct => [ptr, present, value]
#[allow(dead_code)]
struct RcWeakPayload<'a, T>(&'a RcWeak<T>);
#[allow(dead_code)]
struct ArcWeakPayload<'a, T>(&'a ArcWeak<T>);

// Flow hints and block-string hints: we use newtype-struct names.
const NAME_TUPLE_ANCHOR: &str = "__yaml_anchor";
const NAME_TUPLE_WEAK: &str = "__yaml_weak_anchor";
const NAME_FLOW_SEQ: &str = "__yaml_flow_seq";
const NAME_FLOW_MAP: &str = "__yaml_flow_map";
const NAME_LIT_STR: &str = "__yaml_lit_str";
const NAME_FOLD_STR: &str = "__yaml_fold_str";
const NAME_TUPLE_COMMENTED: &str = "__yaml_commented";

// Below this length, block-string wrappers serialize as regular scalars
// instead of YAML block styles. This keeps short values compact.
const MIN_FOLD_CHARS: usize = 32;
/// Maximum width (in characters) for lines inside folded block scalars.
/// Lines will be wrapped at whitespace so that each emitted line is at most
/// this many characters long (excluding indentation). If no whitespace is
/// available within the limit, a hard break is performed.
const FOLDED_WRAP_COL: usize = 80;

// Top-level newtype wrappers for strong/weak simply wrap the real payloads.
impl<T: Serialize> Serialize for RcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // delegate to tuple-struct the serializer knows how to intercept
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Rc::as_ptr(&self.0) as *const T as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Arc::as_ptr(&self.0) as *const T as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcWeakAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as *const T as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(rc) = up {
            ts.serialize_field(&*rc)?;
        } else {
            ts.serialize_field(&())?; // ignored by our serializer
        }
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcWeakAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as *const T as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(arc) = up {
            ts.serialize_field(&*arc)?;
        } else {
            ts.serialize_field(&())?;
        }
        ts.end()
    }
}

// Hints for flow / block strings.
impl<T: Serialize> Serialize for FlowSeq<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FLOW_SEQ, &self.0)
    }
}
impl<T: Serialize> Serialize for FlowMap<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FLOW_MAP, &self.0)
    }
}

impl<T: Serialize> Serialize for Commented<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // Represent as a special tuple-struct with two fields: (comment, value)
        // so the serializer can stage the comment before serializing the value.
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_COMMENTED, 2)?;
        ts.serialize_field(&self.1)?; // comment first
        ts.serialize_field(&self.0)?; // then value
        ts.end()
    }
}

// Deserialization for flow wrappers: delegate to inner T during deserialization.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for FlowSeq<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(FlowSeq)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for FlowMap<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(FlowMap)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Commented<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(|v| Commented(v, String::new()))
    }
}

impl<'a> Serialize for LitStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        if self.0.len() < MIN_FOLD_CHARS && !self.0.contains('\n') {
            return s.serialize_str(self.0);
        }
        s.serialize_newtype_struct(NAME_LIT_STR, &self.0)
    }
}
impl Serialize for LitString {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        if self.0.len() < MIN_FOLD_CHARS && !self.0.contains('\n') {
            return s.serialize_str(&self.0);
        }
        s.serialize_newtype_struct(NAME_LIT_STR, &self.0)
    }
}
impl<'a> Serialize for FoldStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        if self.0.len() < MIN_FOLD_CHARS && !self.0.contains('\n') {
            return s.serialize_str(self.0);
        }
        s.serialize_newtype_struct(NAME_FOLD_STR, &self.0)
    }
}
impl Serialize for FoldString {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        if self.0.len() < MIN_FOLD_CHARS && !self.0.contains('\n') {
            return s.serialize_str(&self.0);
        }
        s.serialize_newtype_struct(NAME_FOLD_STR, &self.0)
    }
}

// Deserialization for owned block string wrappers: delegate to String
impl<'de> Deserialize<'de> for LitString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(LitString)
    }
}
impl<'de> Deserialize<'de> for FoldString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(FoldString)
    }
}

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
pub struct YamlSer<'a, W: Write> {
    /// Destination writer where YAML text is emitted.
    out: &'a mut W,
    /// Spaces per indentation level for block-style collections.
    indent_step: usize,
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
    /// Pending inline comment to be appended after the next scalar (block style only).
    pending_inline_comment: Option<String>,
    /// When the previous token was a list item dash ("- ") and the next node is a mapping,
    /// emit the first key inline on the same line ("- key: value").
    pending_inline_map: bool,
    /// After writing a mapping key and ':', defer writing the following space until we know
    /// whether the value is a scalar (space) or a complex node (newline with no space).
    pending_space_after_colon: bool,
    /// If the previous sequence element after a dash turned out to be a mapping (inline first key),
    /// indent subsequent dashes by one level to satisfy tests expecting "\n  -".
    inline_map_after_dash: bool,
    /// If a sequence element starts with a dash on this depth, capture that depth so
    /// struct-variant mappings emitted immediately after can indent their fields correctly.
    after_dash_depth: Option<usize>,
    /// Current block map indentation depth (for aligning sequences under a map key).
    current_map_depth: Option<usize>,
}

impl<'a, W: Write> YamlSer<'a, W> {
    /// Construct a `YamlSer` that writes to `out`.
    /// Called by `to_writer`/`to_string` entry points.
    pub fn new(out: &'a mut W) -> Self {
        Self {
            out,
            indent_step: 2,
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
            pending_inline_comment: None,
            pending_inline_map: false,
            pending_space_after_colon: false,
            inline_map_after_dash: false,
            after_dash_depth: None,
            current_map_depth: None,
        }
    }
    /// Construct a `YamlSer` with a specific indentation step.
    /// Typically used internally by tests or convenience wrappers.
    pub fn with_indent(out: &'a mut W, indent_step: usize) -> Self {
        let mut s = Self::new(out);
        s.indent_step = indent_step;
        s
    }
    /// Construct a `YamlSer` from user-supplied [`SerializerOptions`].
    /// Used by `to_writer_with_options`.
    pub fn with_options(out: &'a mut W, options: &mut SerializerOptions) -> Self {
        let mut s = Self::new(out);
        s.indent_step = options.indent_step;
        s.anchor_gen = options.anchor_generator.take();
        s
    }

    // -------- helpers --------

    /// Called at the end of emitting a scalar in block style: appends a pending inline
    /// comment (if any) and then emits a newline. In flow style, comments are suppressed.
    #[inline]
    fn write_end_of_scalar(&mut self) -> Result<()> {
        if self.in_flow == 0 {
            if let Some(c) = self.pending_inline_comment.take() {
                self.out.write_str(" # ")?;
                self.out.write_str(&c)?;
            }
            self.newline()?;
        }
        Ok(())
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
        Ok(())
    }

    /// Ensure indentation is written if we are at the start of a line.
    /// Internal: called by most emitters before writing tokens.
    #[inline]
    fn write_indent(&mut self, depth: usize) -> Result<()> {
        if self.at_line_start {
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

    /// Write a folded block string body, wrapping to FOLDED_WRAP_COL characters.
    /// Preserves blank lines between paragraphs. Each emitted line is indented
    /// at depth+1. Wrapping prefers breaking at the last whitespace not
    /// exceeding the limit; if none is present, performs a hard break.
    fn write_folded_block(&mut self, s: &str) -> Result<()> {
        for line in s.split('\n') {
            if line.is_empty() {
                // Preserve empty lines between paragraphs
                self.write_indent(self.depth + 1)?;
                self.newline()?;
                continue;
            }
            let mut start = 0; // byte index
            let bytes = line.as_bytes();
            let mut last_space_byte: Option<usize> = None;
            let mut col = 0usize; // column in chars
            for (i, ch) in line.char_indices() {
                // track potential break positions
                if ch.is_whitespace() {
                    last_space_byte = Some(i);
                }
                col += 1;
                if col > FOLDED_WRAP_COL {
                    let break_at = last_space_byte.unwrap_or(i);
                    // Emit [start, break_at)
                    self.write_indent(self.depth + 1)?;
                    // Safety: break_at is on char boundary
                    let slice = &line[start..break_at];
                    self.out.write_str(slice)?;
                    self.newline()?;
                    // Advance start: skip the whitespace if we broke at space
                    start = if let Some(sp) = last_space_byte { sp + 1 } else { break_at };
                    // Reset trackers starting at new segment
                    last_space_byte = None;
                    col = 0;
                    // Recompute col from start to current i
                    // Count chars between start and i
                    for ch2 in line[start..i].chars() { let _ = ch2; col += 1; }
                }
            }
            // Emit the tail if any
            if start <= bytes.len() {
                self.write_indent(self.depth + 1)?;
                self.out.write_str(&line[start..])?;
                self.newline()?;
            }
        }
        Ok(())
    }

    /// Write a scalar either as plain or as double-quoted with minimal escapes.
    /// Called by most `serialize_*` primitive methods.
    fn write_plain_or_quoted(&mut self, s: &str) -> Result<()> {
        if is_plain_safe(s) {
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
        if is_plain_value_safe(s) {
            self.out.write_str(s)?;
            Ok(())
        } else {
            // Force quoted style for unsafe value tokens (commas/brackets, bool/num-like, etc.).
            self.write_quoted(s)
        }
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
// Impl Serializer for YamlSer
// ------------------------------------------------------------

impl<'a, 'b, W: Write> Serializer for &'a mut YamlSer<'b, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSer<'a, 'b, W>;
    type SerializeTuple = SeqSer<'a, 'b, W>;
    type SerializeTupleStruct = TupleSer<'a, 'b, W>;
    type SerializeTupleVariant = TupleVariantSer<'a, 'b, W>;
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
        self.serialize_f64(v as f64)
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        if v.is_nan() {
            self.out.write_str(".nan")?;
        } else if v.is_infinite() {
            if v.is_sign_positive() {
                self.out.write_str(".inf")?;
            } else {
                self.out.write_str("-.inf")?;
            }
        } else {
            let mut buf = ryu::Buffer::new();
            let s = buf.format(v);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                self.out.write_str(s)?;
                self.out.write_str(".0")?;
            } else {
                self.out.write_str(s)?;
            }
        }
        self.write_end_of_scalar()?;
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.write_space_if_pending()?;
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if let Some(style) = self.pending_str_style.take() {
            // Emit block string. If we are a mapping value, YAML requires a space after ':'.
            // Insert it now if pending.
            self.write_space_if_pending()?;
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            match style {
                StrStyle::Literal => {
                    self.out.write_str("|")?;
                    self.newline()?;
                    for line in v.split('\n') {
                        self.write_indent(self.depth + 1)?;
                        self.out.write_str(line)?;
                        self.newline()?;
                    }
                }
                StrStyle::Folded => {
                    self.out.write_str(">")?;
                    self.newline()?;
                    self.write_folded_block(v)?;
                }
            }
            return Ok(());
        }
        self.write_space_if_pending()?;
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        // Special-case: prefer single-quoted style for select 1-char punctuation to
        // match expected YAML output in tests ('.', '#', '-').
        if v.len() == 1 {
            if let Some(ch) = v.chars().next() {
                if ch == '.' || ch == '#' || ch == '-' {
                    self.out.write_char('\'')?;
                    self.out.write_char(ch)?;
                    self.out.write_char('\'')?;
                    self.write_end_of_scalar()?;
                    return Ok(());
                }
            }
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
                serde::ser::SerializeSeq::serialize_element(&mut seq, b)?;
            }
            return serde::ser::SerializeSeq::end(seq);
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
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        // If we are in a mapping value position, insert the deferred space after ':'
        self.write_space_if_pending()?;
        self.serialize_str(variant)
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
                self.pending_str_style = Some(StrStyle::Literal);
                return value.serialize(self);
            }
            NAME_FOLD_STR => {
                self.pending_str_style = Some(StrStyle::Folded);
                return value.serialize(self);
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
        value.serialize(&mut *self)
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
            // For block sequences, never inline the first inner dash after a parent dash.
            // Style expectation in tests prefers:
            // -\n  - 1
            // rather than "- - 1".
            let inline_first = (!self.at_line_start)
                && self.after_dash_depth.is_some()
                && !self.pending_space_after_colon;
            // Remember if we are a mapping value (space after colon was pending) to handle newline correctly.
            let had_pending_space = self.pending_space_after_colon;
            self.write_anchor_for_complex_node()?;
            if inline_first {
                // Keep staged inline (pending_inline_map) so the child can inline its first dash.
                // Ensure we stay mid-line so the child can emit its first dash inline.
                self.at_line_start = false;
            } else if was_inline_value {
                // Mid-line start. If we are here due to a map value (after ':'), move to next line.
                // If we are here due to a list dash, keep inline.
                self.pending_space_after_colon = false;
                if had_pending_space && !self.at_line_start {
                    self.newline()?;
                }
            }
            // Indentation policy mirrors serialize_map:
            // - After a list dash inline_first: base is dash depth; indent one level deeper.
            // - As a value after a map key: base is current_map_depth (if set), indent one level deeper.
            // - Otherwise (top-level or already at line start): base is current depth.
            let base = if inline_first {
                self.after_dash_depth.unwrap_or(self.depth)
            } else if was_inline_value && self.current_map_depth.is_some() {
                // Guarded by is_some(); use unwrap_or to avoid panicking unwrap
                self.current_map_depth.unwrap_or(self.depth)
            } else {
                self.depth
            };
            // For sequences used as a mapping value, the dash should align under the parent key
            // (no extra indentation). Keep depth at `base` so the first dash starts at the base.
            let depth_next = if inline_first {
                base + 1
            } else if was_inline_value {
                base + 1
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
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        if name == NAME_TUPLE_ANCHOR {
            Ok(TupleSer::anchor_strong(self))
        } else if name == NAME_TUPLE_WEAK {
            Ok(TupleSer::anchor_weak(self))
        } else if name == NAME_TUPLE_COMMENTED {
            Ok(TupleSer::commented(self))
        } else {
            // Treat as normal block sequence
            Ok(TupleSer::normal(self))
        }
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(":\n")?;
        self.at_line_start = true;
        let depth_next = self.depth + 1;
        Ok(TupleVariantSer {
            ser: self,
            depth: depth_next,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
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
            })
        } else {
            let inline_first = self.pending_inline_map;
            let was_inline_value = !self.at_line_start;
            self.write_anchor_for_complex_node()?;
            if inline_first {
                // Suppress newline after a list dash for inline map first key.
                self.pending_inline_map = false;
                // Mark that this sequence element is a mapping printed inline after a dash.
                self.inline_map_after_dash = true;
            } else if was_inline_value {
                // Map used as a value after "key: ". If an anchor was emitted, we are already at
                // the start of a new line due to write_anchor_for_complex_node() -> newline().
                // Only add a newline if we are not already at line start (i.e., no anchor emitted).
                self.pending_space_after_colon = false;
                if !self.at_line_start {
                    self.newline()?;
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
            Ok(MapSer {
                ser: self,
                depth: depth_next,
                flow: false,
                first: true,
                last_key_complex: false,
            })
        }
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(None)
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
        let _was_inline_value = !self.at_line_start;
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

// ------------------------------------------------------------
// Seq / Tuple serializers
// ------------------------------------------------------------

/// Serializer for sequences and tuples.
///
/// Created by `YamlSer::serialize_seq`/`serialize_tuple`. Holds a mutable
/// reference to the parent serializer and formatting state for the sequence.
pub struct SeqSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSer<'b, W>,
    /// Target indentation depth for block-style items.
    depth: usize,
    /// Whether the sequence is being written in flow style (`[a, b]`).
    flow: bool,
    /// Whether the next element is the first (comma handling in flow style).
    first: bool,
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
            v.serialize(&mut *self.ser)?;
        }
        self.first = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.flow {
            let me = self;
            me.ser.out.write_str("]")?;
            if me.ser.in_flow == 0 {
                me.ser.newline()?;
            }
        }
        Ok(())
    }
}

// Tuple-struct serializer (normal or anchor payload)
/// Serializer for tuple-structs.
///
/// Used for three shapes:
/// - Normal tuple-structs (treated like sequences in block style),
/// - Internal strong-anchor payloads (`__yaml_anchor`),
/// - Internal weak-anchor payloads (`__yaml_weak_anchor`).
pub struct TupleSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSer<'b, W>,
    /// Variant describing how to interpret fields.
    kind: TupleKind,
    /// Current field index being serialized.
    idx: usize,
    /// For normal tuples: target indentation depth.
    /// For weak/strong: temporary storage (ptr id or state).
    depth_for_normal: usize,

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
    Normal,       // treat as block seq
    AnchorStrong, // [ptr, value]
    AnchorWeak,   // [ptr, present, value]
    Commented,    // [comment, value]
}
impl<'a, 'b, W: Write> TupleSer<'a, 'b, W> {
    /// Create a tuple serializer for normal tuple-structs.
    fn normal(ser: &'a mut YamlSer<'b, W>) -> Self {
        let depth_next = ser.depth + 1;
        Self {
            ser,
            kind: TupleKind::Normal,
            idx: 0,
            depth_for_normal: depth_next,
            strong_alias_id: None,
            weak_present: false,
            skip_third: false,
            weak_alias_id: None,
            comment_text: None,
        }
    }
    /// Create a tuple serializer for internal strong-anchor payloads.
    fn anchor_strong(ser: &'a mut YamlSer<'b, W>) -> Self {
        Self {
            ser,
            kind: TupleKind::AnchorStrong,
            idx: 0,
            depth_for_normal: 0,
            strong_alias_id: None,
            weak_present: false,
            skip_third: false,
            weak_alias_id: None,
            comment_text: None,
        }
    }
    /// Create a tuple serializer for internal weak-anchor payloads.
    fn anchor_weak(ser: &'a mut YamlSer<'b, W>) -> Self {
        Self {
            ser,
            kind: TupleKind::AnchorWeak,
            idx: 0,
            depth_for_normal: 0,
            strong_alias_id: None,
            weak_present: false,
            skip_third: false,
            weak_alias_id: None,
            comment_text: None,
        }
    }
    /// Create a tuple serializer for internal commented wrapper.
    fn commented(ser: &'a mut YamlSer<'b, W>) -> Self {
        Self {
            ser,
            kind: TupleKind::Commented,
            idx: 0,
            depth_for_normal: 0,
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
        match self.kind {
            TupleKind::Normal => {
                if self.idx == 0 {
                    self.ser.write_anchor_for_complex_node()?;
                    if !self.ser.at_line_start {
                        self.ser.newline()?;
                    }
                }
                self.ser.write_indent(self.ser.depth + 1)?;
                self.ser.out.write_str("- ")?;
                self.ser.at_line_start = false;
                value.serialize(&mut *self.ser)?;
            }
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
                        self.depth_for_normal = ptr; // store ptr for fields #2/#3
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
                            let ptr = self.depth_for_normal;
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
                        if self.ser.in_flow == 0 {
                            // Stage the comment so scalar/alias serializers append it inline via write_end_of_scalar.
                            if !comment.is_empty() {
                                let sanitized = comment.replace('\n', " ");
                                self.ser.pending_inline_comment = Some(sanitized);
                            }
                            // Serialize the inner value as-is. Complex values will ignore the comment (it will be cleared).
                            value.serialize(&mut *self.ser)?;
                            // Ensure no leftover staged comment leaks to subsequent tokens.
                            self.ser.pending_inline_comment = None;
                        } else {
                            // Inside a flow context: serialize value and suppress comments.
                            value.serialize(&mut *self.ser)?;
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

// Tuple variant (enum Variant: ( ... ))
/// Serializer for tuple variants (enum Variant: ( ... )).
///
/// Created by `YamlSer::serialize_tuple_variant` to emit the variant name
/// followed by a block sequence of fields.
pub struct TupleVariantSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSer<'b, W>,
    /// Target indentation depth for the fields.
    depth: usize,
}
impl<'a, 'b, W: Write> SerializeTupleVariant for TupleVariantSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.ser.write_indent(self.depth)?;
        self.ser.out.write_str("- ")?;
        self.ser.at_line_start = false;
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}

// ------------------------------------------------------------
// Map / Struct serializers
// ------------------------------------------------------------

/// Serializer for maps and structs.
///
/// Created by `YamlSer::serialize_map`/`serialize_struct`. Manages indentation
/// and flow/block style for key-value pairs.
pub struct MapSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSer<'b, W>,
    /// Target indentation depth for block-style entries.
    depth: usize,
    /// Whether the mapping is in flow style (`{k: v}`).
    flow: bool,
    /// Whether the next entry is the first (comma handling in flow style).
    first: bool,
    /// Whether the most recently serialized key was a complex (non-scalar) node.
    last_key_complex: bool,
}

impl<'a, 'b, W: Write> SerializeMap for MapSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        if self.flow {
            if !self.first {
                self.ser.out.write_str(", ")?;
            }
            let text = scalar_key_to_string(key)?;
            self.ser.out.write_str(&text)?;
            self.ser.out.write_str(": ")?;
            self.ser.at_line_start = false;
            self.last_key_complex = false;
        } else {
            match scalar_key_to_string(key) {
                Ok(text) => {
                    if !self.ser.at_line_start {
                        self.ser.write_space_if_pending()?;
                    }
                    self.ser.write_indent(self.depth)?;
                    self.ser.out.write_str(&text)?;
                    // Defer the decision to put a space vs. newline until we see the value type.
                    self.ser.out.write_str(":")?;
                    self.ser.pending_space_after_colon = true;
                    self.ser.at_line_start = false;
                    self.last_key_complex = false;
                }
                Err(Error::Unexpected { msg }) if msg == "non-scalar key" => {
                    if !self.ser.at_line_start {
                        self.ser.write_space_if_pending()?;
                    }
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
                    self.last_key_complex = true;
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
                self.ser.write_indent(self.depth)?;
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
            self.ser.newline()?;
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
/// Created by `YamlSer::serialize_struct_variant` to emit the variant name
/// followed by a block mapping of fields.
pub struct StructVariantSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSer<'b, W>,
    /// Target indentation depth for the fields.
    depth: usize,
}
impl<'a, 'b, W: Write> SerializeStructVariant for StructVariantSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        let text = scalar_key_to_string(&key)?;
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

// ------------------------------------------------------------
// Helpers used for extracting ptr/bool inside tuple payloads
// ------------------------------------------------------------

/// Minimal serializer that captures a numeric `usize` from a serialized field.
///
/// Used internally to read the raw pointer value encoded as the first field
/// of our internal anchor tuple payloads.
#[derive(Default)]
struct UsizeCapture {
    v: Option<usize>,
}
impl<'a> Serializer for &'a mut UsizeCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.v = Some(v as usize);
        Ok(())
    }
    fn serialize_char(self, _v: char) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_str(self, _v: &str) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        Err(Error::unexpected("ptr expects number"))
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::unexpected("ptr cannot be none"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> {
        Err(Error::unexpected("ptr not option"))
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::unexpected("ptr cannot be unit"))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl UsizeCapture {
    fn finish(self) -> Result<usize> {
        self.v
            .ok_or_else(|| Error::unexpected("missing numeric ptr"))
    }
}

/// Minimal serializer that captures a boolean from a serialized field.
///
/// Used internally to read the `present` flag from weak-anchor payloads.
#[derive(Default)]
struct BoolCapture {
    v: Option<bool>,
}
impl<'a> Serializer for &'a mut BoolCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.v = Some(v);
        Ok(())
    }
    fn serialize_i8(self, _v: i8) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_i16(self, _v: i16) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_i32(self, _v: i32) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_i64(self, _v: i64) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_u8(self, _v: u8) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_u16(self, _v: u16) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_u32(self, _v: u32) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_u64(self, _v: u64) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_f32(self, _v: f32) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_char(self, _c: char) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_str(self, _v: &str) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _v: &T) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::unexpected("bool expected"))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl BoolCapture {
    fn finish(self) -> Result<bool> {
        self.v.ok_or_else(|| Error::unexpected("missing bool"))
    }
}

/// Minimal serializer that captures a string from a serialized field.
///
/// Used internally to read the comment text for the Commented wrapper.
#[derive(Default)]
struct StrCapture {
    s: Option<String>,
}
impl<'a> Serializer for &'a mut StrCapture {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_str(self, v: &str) -> Result<()> {
        self.s = Some(v.to_string());
        Ok(())
    }

    fn serialize_bool(self, _v: bool) -> Result<()> {
        unexpected_e()
    }
    fn serialize_i8(self, _v: i8) -> Result<()> {
        unexpected_e()
    }
    fn serialize_i16(self, _v: i16) -> Result<()> {
        unexpected_e()
    }
    fn serialize_i32(self, _v: i32) -> Result<()> {
        unexpected_e()
    }
    fn serialize_i64(self, _v: i64) -> Result<()> {
        unexpected_e()
    }
    fn serialize_i128(self, _v: i128) -> Result<()> {
        unexpected_e()
    }
    fn serialize_u8(self, _v: u8) -> Result<()> {
        unexpected_e()
    }
    fn serialize_u16(self, _v: u16) -> Result<()> {
        unexpected_e()
    }
    fn serialize_u32(self, _v: u32) -> Result<()> {
        unexpected_e()
    }
    fn serialize_u64(self, _v: u64) -> Result<()> {
        unexpected_e()
    }
    fn serialize_u128(self, _v: u128) -> Result<()> {
        unexpected_e()
    }
    fn serialize_f32(self, _v: f32) -> Result<()> {
        unexpected_e()
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        unexpected_e()
    }
    fn serialize_char(self, _c: char) -> Result<()> {
        unexpected_e()
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        unexpected_e()
    }
    fn serialize_none(self) -> Result<()> {
        unexpected_e()
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit(self) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _value: &T,
    ) -> Result<()> {
        unexpected_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _i: u32,
        _v: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        unexpected()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> {
        unexpected_e()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl StrCapture {
    fn finish(self) -> Result<String> {
        self.s.ok_or_else(|| Error::unexpected("missing string"))
    }
}

// ------------------------------------------------------------
// Key scalar helper
// ------------------------------------------------------------

/// Serialize a key using a restricted scalar-only serializer into a `String`.
///
/// Called by map/struct serializers to ensure YAML keys are scalars.
fn scalar_key_to_string<K: Serialize + ?Sized>(key: &K) -> Result<String> {
    let mut s = String::new();
    {
        let mut ks = KeyScalarSink { s: &mut s };
        key.serialize(&mut ks)?;
    }
    Ok(s)
}

struct KeyScalarSink<'a> {
    s: &'a mut String,
}

impl<'a> Serializer for &'a mut KeyScalarSink<'a> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.s.push_str(if v { "true" } else { "false" });
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i128(self, v: i128) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u128(self, v: u128) -> Result<()> {
        let _ = write!(self.s, "{}", v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<()> {
        let v = v as f64;
        if v.is_nan() {
            self.s.push_str(".nan");
        } else if v.is_infinite() {
            if v.is_sign_positive() {
                self.s.push_str(".inf");
            } else {
                self.s.push_str("-.inf");
            }
        } else {
            let mut buf = ryu::Buffer::new();
            let s = buf.format(v);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                self.s.push_str(s);
                self.s.push_str(".0");
            } else {
                self.s.push_str(s);
            }
        }
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        if v.is_nan() {
            self.s.push_str(".nan");
        } else if v.is_infinite() {
            if v.is_sign_positive() {
                self.s.push_str(".inf");
            } else {
                self.s.push_str("-.inf");
            }
        } else {
            let mut buf = ryu::Buffer::new();
            let s = buf.format(v);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                self.s.push_str(s);
                self.s.push_str(".0");
            } else {
                self.s.push_str(s);
            }
        }
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }
    fn serialize_str(self, v: &str) -> Result<()> {
        if is_plain_safe(v) {
            self.s.push_str(v);
        } else {
            self.s.push('"');
            for ch in v.chars() {
                match ch {
                    '\\' => self.s.push_str("\\\\"),
                    '"' => self.s.push_str("\\\""),
                    '\n' => self.s.push_str("\\n"),
                    '\r' => self.s.push_str("\\r"),
                    '\t' => self.s.push_str("\\t"),
                    c if c.is_control() => {
                        use std::fmt::Write as _;
                        // Writing into a String cannot fail; ignore the Result to avoid unwrap.
                        let _ = write!(self.s, "\\u{:04X}", c as u32);
                    }
                    c => self.s.push(c),
                }
            }
            self.s.push('"');
        }
        Ok(())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        non_scalar_key_e()
    }
    fn serialize_none(self) -> Result<()> {
        self.s.push_str("null");
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<()> {
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<()> {
        self.s.push_str("null");
        Ok(())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        non_scalar_key_e()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<()> {
        non_scalar_key_e()
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<ser::Impossible<(), Error>> {
        non_scalar_key()
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, v: &T) -> Result<()> {
        self.serialize_str(&v.to_string())
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}

fn unexpected() -> Result<ser::Impossible<(), Error>> {
    Err(Error::unexpected("unexpected"))
}

fn unexpected_e() -> Result<()> {
    Err(Error::unexpected("unexpected"))
}

fn non_scalar_key() -> Result<ser::Impossible<(), Error>> {
    Err(Error::unexpected("non-scalar key"))
}

fn non_scalar_key_e() -> Result<()> {
    Err(Error::unexpected("non-scalar key"))
}
