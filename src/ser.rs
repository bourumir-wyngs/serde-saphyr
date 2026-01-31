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
use serde::ser::Error as _;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Mutex, Weak as ArcWeak};

use crate::long_strings::{NAME_FOLD_STR, NAME_LIT_STR};
use crate::serializer_options::{FOLDED_WRAP_CHARS, MIN_FOLD_CHARS, SerializerOptions};
use crate::{
    ArcAnchor, ArcRecursion, ArcRecursive, ArcWeakAnchor, RcAnchor, RcRecursion, RcRecursive,
    RcWeakAnchor, zmij_format,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
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

/// Add an empty line after the wrapped value when serializing.
///
/// This wrapper is transparent during deserialization and can be nested with
/// other wrappers like `Commented`, `FlowSeq`, etc.
/// ```rust
/// use serde::Serialize;
/// use serde_saphyr::SpaceAfter;
///
/// #[derive(Serialize)]
/// struct Config {
///     first: SpaceAfter<i32>,
///     second: i32,
/// }
///
/// let cfg = Config { first: SpaceAfter(1), second: 2 };
/// let yaml = serde_saphyr::to_string(&cfg).unwrap();
/// // The output will have an empty line after "first: 1"
/// ```
/// **Important:** Avoid using this wrapper with `LitStr`/`LitString` as it may add the empty
/// line to the string content. For `FoldStr`/`FoldString` and other YAML values
/// (e.g. `key: value`, quoted scalars), the extra empty line is cosmetic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceAfter<T>(pub T);

/// Attach an inline YAML comment to a value when serializing.
///
/// This wrapper lets you annotate a scalar with an inline YAML comment that is
/// emitted after the value when using block style. The typical form is:
/// `value # comment`. This is the most useful when deserializing the anchor
/// reference (so a human reader can see what a referenced value represents).
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
/// struct S { xn: Commented<i32> }
///
/// let s = S { xn: Commented(5, "send five starships first".into()) };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "xn: 5 # send five starships first\n");
/// ```
///
/// *Important*: Comments are suppressed in flow contexts (no `#` appears), and
/// ignored for complex inner values. Value with `Commented` wrapper will be
/// deserialized correctly as well, but deserializing comments is currently not
/// supported.
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commented<T>(pub T, pub String);

pub use crate::long_strings::{FoldStr, FoldString, LitStr, LitString};

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

// Recursive weak payloads: defer locking/borrowing until actual value serialization.
struct RcRecursivePayload<'a, T>(&'a Rc<RefCell<Option<T>>>);
struct ArcRecursivePayload<'a, T>(&'a Arc<Mutex<Option<T>>>);

impl<T: Serialize> Serialize for RcRecursivePayload<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let borrowed = self.0.borrow();
        if let Some(value) = borrowed.as_ref() {
            value.serialize(s)
        } else {
            s.serialize_unit()
        }
    }
}

impl<T: Serialize> Serialize for ArcRecursivePayload<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let guard = self
            .0
            .lock()
            .map_err(|_| ser::Error::custom("recursive Arc anchor mutex poisoned"))?;
        if let Some(value) = guard.as_ref() {
            value.serialize(s)
        } else {
            s.serialize_unit()
        }
    }
}

// Flow hints and block-string hints: we use newtype-struct names.
const NAME_TUPLE_ANCHOR: &str = "__yaml_anchor";
const NAME_TUPLE_WEAK: &str = "__yaml_weak_anchor";
const NAME_FLOW_SEQ: &str = "__yaml_flow_seq";
const NAME_FLOW_MAP: &str = "__yaml_flow_map";
const NAME_TUPLE_COMMENTED: &str = "__yaml_commented";
const NAME_SPACE_AFTER: &str = "__yaml_space_after";

// Top-level newtype wrappers for strong/weak simply wrap the real payloads.
impl<T: Serialize> Serialize for RcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // delegate to tuple-struct the serializer knows how to intercept
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Rc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Arc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&*self.0)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcRecursive<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Rc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        let borrowed = self.0.borrow();
        let value = borrowed
            .as_ref()
            .ok_or_else(|| ser::Error::custom("recursive Rc anchor not initialized"))?;
        ts.serialize_field(value)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcRecursive<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_ANCHOR, 2)?;
        let ptr = Arc::as_ptr(&self.0) as usize;
        ts.serialize_field(&ptr)?;
        let guard = self
            .0
            .lock()
            .map_err(|_| ser::Error::custom("recursive Arc anchor mutex poisoned"))?;
        let value = guard
            .as_ref()
            .ok_or_else(|| ser::Error::custom("recursive Arc anchor not initialized"))?;
        ts.serialize_field(value)?;
        ts.end()
    }
}
impl<T: Serialize> Serialize for RcWeakAnchor<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
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
        let ptr = self.0.as_ptr() as usize;
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
impl<T: Serialize> Serialize for RcRecursion<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(rc) = up {
            ts.serialize_field(&RcRecursivePayload(&rc))?;
        } else {
            ts.serialize_field(&())?;
        }
        ts.end()
    }
}
impl<T: Serialize> Serialize for ArcRecursion<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let up = self.0.upgrade();
        let mut ts = s.serialize_tuple_struct(NAME_TUPLE_WEAK, 3)?;
        let ptr = self.0.as_ptr() as usize;
        ts.serialize_field(&ptr)?;
        ts.serialize_field(&up.is_some())?;
        if let Some(arc) = up {
            ts.serialize_field(&ArcRecursivePayload(&arc))?;
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
impl<T: Serialize> Serialize for SpaceAfter<T> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_SPACE_AFTER, &self.0)
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
impl<'de, T: Deserialize<'de>> Deserialize<'de> for SpaceAfter<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(SpaceAfter)
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
///
/// This type is also re-exported from the crate root as [`serde_saphyr::Serializer`].
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
    /// If true, emit YAML tags for simple enums that serialize to a single scalar.
    tagged_enums: bool,
    /// If true, empty maps are emitted as {} and lists as []
    empty_as_braces: bool,
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
            tagged_enums: false,
            empty_as_braces: true,
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
        s.prefer_block_scalars = options.prefer_block_scalars;
        s.quote_all = options.quote_all;
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
                || c.is_control() // control chars (includes \n, \t, \r, etc.) need escaping
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
        crate::wrapping::write_folded_block(
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
        } else if is_plain_value_safe(s, self.yaml_12) {
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
            char::from_digit(indent_n as u32, 10).ok_or_else(|| {
                Error::custom("indentation indicator must be a single digit (1..=9)")
            })
        }

        // If no explicit style pending, and option is enabled, auto-select block style
        // similar to LitStr/FoldStr wrappers to improve compatibility with Go's yaml.v3.
        // However, DISABLE auto block scalars when the string needs quoting as a value
        // (per ser_quoting::is_plain_value_safe), unless the only reason it needs quoting
        // is the presence of newlines themselves. This ensures cases like "hey:\n" remain
        // quoted (because trimmed value ends with ':'), even when prefer_block_scalars=true.
        // Also skip block scalars when quote_all is enabled - use quoted strings instead.
        if self.pending_str_style.is_none()
            && self.prefer_block_scalars
            && self.in_flow == 0
            && !self.quote_all
        {
            use crate::ser_quoting::is_plain_value_safe;

            if v.contains('\n') {
                // If removing newlines makes it plain-safe, then the only problem was newlines →
                // allow literal block style. Otherwise, don't auto-select block style so that
                // quoting logic handles it (e.g., values ending with ':').
                let trimmed = v.trim_end_matches('\n');
                let normalized = trimmed.replace('\n', " ");
                if is_plain_value_safe(&normalized, self.yaml_12) {
                    self.pending_str_style = Some(StrStyle::Literal);
                    self.pending_str_from_auto = true;
                }
            } else {
                // Single-line string. If it needs quoting as a value, don't auto-fold.
                let needs_quoting = !is_plain_value_safe(v, self.yaml_12);
                if !needs_quoting {
                    // Measure in characters, not bytes.
                    if v.len() > self.folded_wrap_col {
                        self.pending_str_style = Some(StrStyle::Folded);
                        self.pending_str_from_auto = true;
                    }
                }
            }
        }
        if let Some(style) = self.pending_str_style.take() {
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
            // Compute the indentation indicator N for block scalars.
            // N = indent_step * body_base = number of spaces the parser will strip.
            // We must emit an explicit indicator when the first non-empty content line
            // has leading whitespace, so the parser knows how much to strip.
            let body_base = base + 1;
            let indent_n = self.indent_step * body_base;

            // Check if we need an explicit indentation indicator.
            // Required when the first non-empty line has leading whitespace.
            let content_trimmed = v.trim_end_matches('\n');
            let first_line_spaces = crate::wrapping::first_line_leading_spaces(content_trimmed);
            let needs_indicator = first_line_spaces > 0;

            // If N > 9, YAML parsers reject it. Fall back to quoting.
            if needs_indicator && indent_n > 9 {
                // Reset state and fall through to quoted string handling
                self.pending_str_style = None;
                self.pending_str_from_auto = false;
                self.write_scalar_prefix_if_anchor()?;
                self.write_plain_or_quoted_value(v)?;
                self.write_end_of_scalar()?;
                return Ok(());
            }

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
                    if needs_indicator {
                        // Write the indentation indicator digit
                        let digit = block_indent_indicator_digit(indent_n)?;
                        self.out.write_char(digit)?;
                    }
                    match trailing_nl {
                        0 => self.out.write_char('-')?,
                        1 => {} // clip is the default, no indicator needed
                        _ => self.out.write_char('+')?,
                    }
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
                    if needs_indicator {
                        // Write the indentation indicator digit
                        let digit = block_indent_indicator_digit(indent_n)?;
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
            // If we are a mapping value (space after colon was pending), we will handle
            // the newline later in SeqSer::serialize_element to keep empty sequences inline.
            self.write_anchor_for_complex_node()?;
            if inline_first {
                // Keep staged inline (pending_inline_map) so the child can inline its first dash.
                // Ensure we stay mid-line so the child can emit its first dash inline.
                self.at_line_start = false;
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
            let depth_next = if inline_first || was_inline_value {
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
                let known_empty = matches!(_len, Some(0));
                let known_non_empty = matches!(_len, Some(n) if n > 0);

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
                && _len.is_none()
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

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(_len))
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
/// Created by `YamlSerializer::serialize_seq`/`serialize_tuple`. Holds a mutable
/// reference to the parent serializer and formatting state for the sequence.
pub struct SeqSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSerializer<'b, W>,
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
        } else if self.first {
            // Empty block-style sequence.
            if self.ser.empty_as_braces {
                // If we were pending a space after a colon (map value position), write it now.
                if self.ser.pending_space_after_colon {
                    self.ser.out.write_str(" ")?;
                    self.ser.pending_space_after_colon = false;
                }
                // If at line start, indent appropriately.
                if self.ser.at_line_start {
                    self.ser.write_indent(self.depth)?;
                }
                self.ser.out.write_str("[]")?;
                self.ser.newline()?;
            } else {
                // Preserve legacy behavior: just emit a newline (empty body).
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

// Tuple-struct serializer (normal or anchor payload)
/// Serializer for tuple-structs.
///
/// Used for three shapes:
/// - Normal tuple-structs (treated like sequences in block style),
/// - Internal strong-anchor payloads (`__yaml_anchor`),
/// - Internal weak-anchor payloads (`__yaml_weak_anchor`).
pub struct TupleSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSerializer<'b, W>,
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
    fn normal(ser: &'a mut YamlSerializer<'b, W>) -> Self {
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
    fn anchor_strong(ser: &'a mut YamlSerializer<'b, W>) -> Self {
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
    fn anchor_weak(ser: &'a mut YamlSerializer<'b, W>) -> Self {
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
    fn commented(ser: &'a mut YamlSerializer<'b, W>) -> Self {
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
/// Created by `YamlSerializer::serialize_tuple_variant` to emit the variant name
/// followed by a block sequence of fields.
pub struct TupleVariantSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSerializer<'b, W>,
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
/// Created by `YamlSerializer::serialize_map`/`serialize_struct`. Manages indentation
/// and flow/block style for key-value pairs.
pub struct MapSer<'a, 'b, W: Write> {
    /// Parent YAML serializer.
    ser: &'a mut YamlSerializer<'b, W>,
    /// Target indentation depth for block-style entries.
    depth: usize,
    /// Whether the mapping is in flow style (`{k: v}`).
    flow: bool,
    /// Whether the next entry is the first (comma handling in flow style).
    first: bool,
    /// Whether the most recently serialized key was a complex (non-scalar) node.
    last_key_complex: bool,
    /// Align continuation lines under an inline-after-dash first key by adding 2 spaces.
    align_after_dash: bool,
    /// If true, this mapping began in a value position and stayed inline (after `key:`)
    /// so that an empty map can be serialized as `{}` right there. When the first key arrives,
    /// we must break the line and indent appropriately.
    inline_value_start: bool,
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

            match scalar_key_to_string(key, self.ser.yaml_12) {
                Ok(text) => {
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
                    self.ser.out.write_str(&text)?;
                    // Defer the decision to put a space vs. newline until we see the value type.
                    self.ser.out.write_str(":")?;
                    self.ser.pending_space_after_colon = true;
                    self.ser.at_line_start = false;
                    self.last_key_complex = false;
                }
                Err(Error::Unexpected { msg }) if msg == "non-scalar key" => {
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
    ser: &'a mut YamlSerializer<'b, W>,
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
impl Serializer for &mut UsizeCapture {
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
impl Serializer for &mut BoolCapture {
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
impl Serializer for &mut StrCapture {
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
fn scalar_key_to_string<K: Serialize + ?Sized>(key: &K, yaml_12: bool) -> Result<String> {
    let mut s = String::new();
    {
        let mut ks = KeyScalarSink { s: &mut s, yaml_12 };
        key.serialize(&mut ks)?;
    }
    Ok(s)
}

struct KeyScalarSink<'a> {
    s: &'a mut String,
    yaml_12: bool,
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
        zmij_format::push_float_string(self.s, v)
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        zmij_format::push_float_string(self.s, v)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }
    fn serialize_str(self, v: &str) -> Result<()> {
        // Keys are in a more restrictive position than values (':' is structural),
        // but they also must avoid ambiguous plain scalars (e.g. YAML 1.1 bool spellings
        // like y/n/yes/no) to preserve intended string keys.
        if is_plain_safe(v) && is_plain_value_safe(v, self.yaml_12) {
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
        value: &T,
    ) -> Result<()> {
        // Treat newtype structs transparently. This allows common key wrappers like
        // `struct Key(String);` / `struct Id(u64);` to be emitted as scalar keys.
        value.serialize(self)
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
