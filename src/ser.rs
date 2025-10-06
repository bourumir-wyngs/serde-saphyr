//! Single-pass YAML serializer with optional anchors for Rc/Arc/Weak,
//! order preservation (uses the iterator order of your types), simple
//! style controls (block strings & flow containers), and special
//! float handling for NaN/±Inf. No intermediate YAML DOM is built.
//
// Usage example:
//
// use serde::Serialize;
// use std::rc::Rc;
// use serde_saphyr::ser::{to_string, RcAnchor, LitStr, FlowSeq};
//
// #[derive(Serialize)]
// struct Cfg {
//     name: String,
//     ports: FlowSeq<Vec<u16>>,   // render `[8080, 8081]`
//     note: LitStr<'static>,      // render as `|` block
//     data: RcAnchor<Vec<i32>>,   // first sight => &a1
//     alias: RcAnchor<Vec<i32>>,  // later sight => *a1
// }
//
// fn main() {
//     let shared = Rc::new(vec![1,2,3]);
//     let cfg = Cfg {
//         name: "demo".into(),
//         ports: FlowSeq(vec![8080, 8081]),
//         note: LitStr("line 1\nline 2"),
//         data: RcAnchor(shared.clone()),
//         alias: RcAnchor(shared),
//     };
//     println!("{}", to_string(&cfg).unwrap());
// }

use serde::ser::{
    self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Weak as ArcWeak};

// ------------------------------------------------------------
// Public API
// ------------------------------------------------------------

/// Serialization error.
#[derive(Debug)]
pub struct Error(String);

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}
impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error(e.to_string())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for Error {}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Serialize a value to a YAML `String`.
pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let mut out = String::new();
    to_writer(&mut out, value)?;
    Ok(out)
}

/// Serialize a value to a writer with default indent (2 spaces).
///
/// # Parameters
/// - `out`: destination that implements `fmt::Write`.
/// - `value`: the value to serialize.
///
/// # Return
/// Returns `Ok(())` if serialization succeeds, otherwise an `Error`.
pub fn to_writer<W: Write, T: Serialize>(out: &mut W, value: &T) -> Result<()> {
    let mut ser = YamlSer::new(out);
    value.serialize(&mut ser)
}

/// Serialize a value to a writer with a custom indent size.
///
/// # Parameters
/// - `out`: destination that implements `fmt::Write`.
/// - `value`: the value to serialize.
/// - `indent_step`: spaces per indentation level.
///
/// # Return
/// Returns `Ok(())` if serialization succeeds, otherwise an `Error`.
pub fn to_writer_with_indent<W: Write, T: Serialize>(
    out: &mut W,
    value: &T,
    indent_step: usize,
) -> Result<()> {
    let mut ser = YamlSer::with_indent(out, indent_step);
    value.serialize(&mut ser)
}

/// Wrap an `Rc<T>` to opt-in to anchor emission for that field.
#[derive(Clone)]
pub struct RcAnchor<T>(pub Rc<T>);
/// Wrap an `Arc<T>` to opt-in to anchor emission for that field.
#[derive(Clone)]
pub struct ArcAnchor<T>(pub Arc<T>);
/// Wrap an `std::rc::Weak<T>` to opt-in; if dangling it serializes as `null`.
#[derive(Clone)]
pub struct RcWeakAnchor<T>(pub RcWeak<T>);
/// Wrap an `std::sync::Weak<T>` to opt-in; if dangling it serializes as `null`.
#[derive(Clone)]
pub struct ArcWeakAnchor<T>(pub ArcWeak<T>);

/// Force a sequence to be emitted in flow style: `[a, b, c]`.
#[derive(Clone)]
pub struct FlowSeq<T>(pub T);
/// Force a mapping to be emitted in flow style: `{k1: v1, k2: v2}`.
#[derive(Clone)]
pub struct FlowMap<T>(pub T);

/// Block literal string (`|`), lines are preserved.
#[derive(Clone, Copy)]
pub struct LitStr<'a>(pub &'a str);
/// Block folded string (`>`), lines are folded by YAML consumers.
#[derive(Clone, Copy)]
pub struct FoldStr<'a>(pub &'a str);

// ------------------------------------------------------------
// Internal wrappers -> shape the stream Serde produces so our
// serializer can intercept them in a single pass.
// ------------------------------------------------------------

// Strong: "__yaml_anchor" tuple-struct => [ptr, value]
struct RcStrongPayload<'a, T>(&'a Rc<T>);
struct ArcStrongPayload<'a, T>(&'a Arc<T>);

// Weak: "__yaml_weak_anchor" tuple-struct => [ptr, present, value]
struct RcWeakPayload<'a, T>(&'a RcWeak<T>);
struct ArcWeakPayload<'a, T>(&'a ArcWeak<T>);

// Flow hints and block-string hints: we use newtype-struct names.
const NAME_TUPLE_ANCHOR: &str = "__yaml_anchor";
const NAME_TUPLE_WEAK: &str = "__yaml_weak_anchor";
const NAME_FLOW_SEQ: &str = "__yaml_flow_seq";
const NAME_FLOW_MAP: &str = "__yaml_flow_map";
const NAME_LIT_STR: &str = "__yaml_lit_str";
const NAME_FOLD_STR: &str = "__yaml_fold_str";

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
impl<'a> Serialize for LitStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_LIT_STR, &self.0)
    }
}
impl<'a> Serialize for FoldStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FOLD_STR, &self.0)
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

pub struct YamlSer<'a, W: Write> {
    out: &'a mut W,
    indent_step: usize,
    depth: usize,
    at_line_start: bool,

    // Anchors:
    anchors: HashMap<usize, String>, // ptr -> "aN"
    next_anchor_id: usize,
    pending_anchor: Option<String>,

    // Style flags:
    pending_flow: Option<PendingFlow>,
    in_flow: usize, // >0 means we're inside a flow container
    pending_str_style: Option<StrStyle>,
}

impl<'a, W: Write> YamlSer<'a, W> {
    pub fn new(out: &'a mut W) -> Self {
        Self {
            out,
            indent_step: 2,
            depth: 0,
            at_line_start: true,
            anchors: HashMap::new(),
            next_anchor_id: 1,
            pending_anchor: None,
            pending_flow: None,
            in_flow: 0,
            pending_str_style: None,
        }
    }
    pub fn with_indent(out: &'a mut W, indent_step: usize) -> Self {
        Self { indent_step, ..Self::new(out) }
    }

    // -------- helpers --------

    fn write_indent(&mut self, depth: usize) -> Result<()> {
        if self.at_line_start {
            for _ in 0..(depth * self.indent_step) {
                self.out.write_char(' ')?;
            }
            self.at_line_start = false;
        }
        Ok(())
    }

    fn newline(&mut self) -> Result<()> {
        self.out.write_char('\n')?;
        self.at_line_start = true;
        Ok(())
    }

    fn write_plain_or_quoted(&mut self, s: &str) -> Result<()> {
        if is_plain_safe(s) {
            self.out.write_str(s)?;
            Ok(())
        } else {
            self.out.write_char('"')?;
            for ch in s.chars() {
                match ch {
                    '\\' => self.out.write_str("\\\\")?,
                    '"' => self.out.write_str("\\\"")?,
                    '\n' => self.out.write_str("\\n")?,
                    '\r' => self.out.write_str("\\r")?,
                    '\t' => self.out.write_str("\\t")?,
                    c if c.is_control() => write!(self.out, "\\u{:04X}", c as u32)?,
                    c => self.out.write_char(c)?,
                }
            }
            self.out.write_char('"')?;
            Ok(())
        }
    }

    fn write_scalar_prefix_if_anchor(&mut self) -> Result<()> {
        if let Some(a) = self.pending_anchor.take() {
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            write!(self.out, "&{} ", a)?;
        }
        Ok(())
    }

    fn write_anchor_for_complex_node(&mut self) -> Result<()> {
        if let Some(a) = self.pending_anchor.take() {
            if self.at_line_start {
                self.write_indent(self.depth)?;
            }
            write!(self.out, "&{}", a)?;
            self.newline()?;
        }
        Ok(())
    }

    fn write_alias(&mut self, name: &str) -> Result<()> {
        if self.at_line_start {
            self.write_indent(self.depth)?;
        }
        write!(self.out, "*{}", name)?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn new_anchor_name(&mut self) -> String {
        let n = self.next_anchor_id;
        self.next_anchor_id += 1;
        format!("a{n}")
    }

    fn define_anchor_and_serialize<T: Serialize + ?Sized>(&mut self, ptr: usize, v: &T) -> Result<()> {
        let name = self.new_anchor_name();
        self.anchors.insert(ptr, name.clone());
        self.pending_anchor = Some(name);
        v.serialize(&mut *self)
    }

    fn alias_or_define_and_serialize<T: Serialize + ?Sized>(&mut self, ptr: usize, v: &T) -> Result<()> {
        if let Some(name) = self.anchors.get(&ptr).cloned() {
            self.write_alias(&name)
        } else {
            self.define_anchor_and_serialize(ptr, v)
        }
    }

    fn take_flow_for_seq(&mut self) -> bool {
        if self.in_flow > 0 {
            true
        } else if let Some(PendingFlow::AnySeq) = self.pending_flow.take() {
            true
        } else {
            false
        }
    }
    fn take_flow_for_map(&mut self) -> bool {
        if self.in_flow > 0 {
            true
        } else if let Some(PendingFlow::AnyMap) = self.pending_flow.take() {
            true
        } else {
            false
        }
    }

    fn with_in_flow<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.in_flow += 1;
        let r = f(self);
        self.in_flow -= 1;
        r
    }
}

// ------------------------------------------------------------
// Scalar helpers
// ------------------------------------------------------------

fn is_plain_safe(s: &str) -> bool {
    if s.is_empty() { return false; }
    if s == "~" || s.eq_ignore_ascii_case("null") || s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("false") { return false; }
    let bytes = s.as_bytes();
    if bytes[0].is_ascii_whitespace()
        || matches!(bytes[0], b'-' | b'?' | b':' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>' | b'\'' | b'"' | b'%' | b'@' | b'`')
    { return false; }
    if s.chars().any(|c| c.is_control()) { return false; }
    if s.contains(':') || s.contains('#') { return false; }
    true
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
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.out.write_str(if v { "true" } else { "false" })?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_i16(self, v: i16) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_i32(self, v: i32) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start { self.write_indent(self.depth)?; }
        write!(self.out, "{}", v)?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_u16(self, v: u16) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_u32(self, v: u32) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start { self.write_indent(self.depth)?; }
        write!(self.out, "{}", v)?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> { self.serialize_f64(v as f64) }
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start { self.write_indent(self.depth)?; }
        if v.is_nan() {
            self.out.write_str(".nan")?;
        } else if v.is_infinite() {
            if v.is_sign_positive() { self.out.write_str(".inf")?; }
            else { self.out.write_str("-.inf")?; }
        } else {
            // ASCII dot for fractions and ASCII minus by default.
            write!(self.out, "{}", v)?;
        }
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if let Some(style) = self.pending_str_style.take() {
            // Emit block string
            if self.at_line_start { self.write_indent(self.depth)?; }
            match style { StrStyle::Literal => self.out.write_str("|")?,
                StrStyle::Folded  => self.out.write_str(">")?, }
            self.newline()?;
            for line in v.split('\n') {
                self.write_indent(self.depth + 1)?;
                self.out.write_str(line)?;
                self.newline()?;
            }
            return Ok(());
        }
        self.write_scalar_prefix_if_anchor()?;
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.write_plain_or_quoted(v)?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for b in v {
            serde::ser::SerializeSeq::serialize_element(&mut seq, b)?;
        }
        serde::ser::SerializeSeq::end(seq)
    }

    fn serialize_none(self) -> Result<()> {
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.out.write_str("null")?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.out.write_str("null")?;
        if self.in_flow == 0 { self.newline()?; }
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self, _name: &'static str, _variant_index: u32, variant: &'static str
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self, name: &'static str, value: &T
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
        mut self, _name: &'static str, _variant_index: u32, variant: &'static str, value: &T
    ) -> Result<()> {
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(": ")?;
        self.at_line_start = false;
        value.serialize(&mut *self)
    }

    // -------- Collections --------

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        let flow = self.take_flow_for_seq();
        if flow {
            self.write_scalar_prefix_if_anchor()?;
            if self.at_line_start { self.write_indent(self.depth)?; }
            self.out.write_str("[")?;
            self.at_line_start = false;
            let depth_next = self.depth; // inline
            Ok(SeqSer { ser: self, depth: depth_next, flow: true, first: true })
        } else {
            self.write_anchor_for_complex_node()?;
            if !self.at_line_start { self.newline()?; }
            let depth_next = self.depth + 1;
            Ok(SeqSer { ser: self, depth: depth_next, flow: false, first: true })
        }
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self, name: &'static str, _len: usize
    ) -> Result<Self::SerializeTupleStruct> {
        if name == NAME_TUPLE_ANCHOR {
            Ok(TupleSer::anchor_strong(self))
        } else if name == NAME_TUPLE_WEAK {
            Ok(TupleSer::anchor_weak(self))
        } else {
            // Treat as normal block sequence
            Ok(TupleSer::normal(self))
        }
    }

    fn serialize_tuple_variant(
        mut self, _name: &'static str, _variant_index: u32, variant: &'static str, _len: usize
    ) -> Result<Self::SerializeTupleVariant> {
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(":\n")?;
        self.at_line_start = true;
        let depth_next = self.depth + 1;
        Ok(TupleVariantSer { ser: self, depth: depth_next })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        let flow = self.take_flow_for_map();
        if flow {
            self.write_scalar_prefix_if_anchor()?;
            if self.at_line_start { self.write_indent(self.depth)?; }
            self.out.write_str("{")?;
            self.at_line_start = false;
            let depth_next = self.depth;
            Ok(MapSer { ser: self, depth: depth_next, flow: true, first: true })
        } else {
            self.write_anchor_for_complex_node()?;
            if !self.at_line_start { self.newline()?; }
            let depth_next = self.depth + 1;
            Ok(MapSer { ser: self, depth: depth_next, flow: false, first: true })
        }
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(None)
    }

    fn serialize_struct_variant(
        mut self, _name: &'static str, _variant_index: u32, variant: &'static str, _len: usize
    ) -> Result<Self::SerializeStructVariant> {
        if self.at_line_start { self.write_indent(self.depth)?; }
        self.write_plain_or_quoted(variant)?;
        self.out.write_str(":\n")?;
        self.at_line_start = true;
        let depth_next = self.depth + 1;
        Ok(StructVariantSer { ser: self, depth: depth_next })
    }
}

// ------------------------------------------------------------
// Seq / Tuple serializers
// ------------------------------------------------------------

pub struct SeqSer<'a, 'b, W: Write> {
    ser: &'a mut YamlSer<'b, W>,
    depth: usize,
    flow: bool,
    first: bool,
}


impl<'a, 'b, W: Write> SerializeTuple for SeqSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<()> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<()> { SerializeSeq::end(self) }
}

// We need our own end() for flow seq; implement Drop-in wrapper:
impl<'a, 'b, W: Write> SeqSer<'a, 'b, W> {
    fn finish(mut self) -> Result<()> {
        if self.flow {
            self.ser.out.write_str("]")?;
            if self.ser.in_flow == 0 { self.ser.newline()?; }
        }
        Ok(())
    }
}

// But trait requires end(self), so fix above:

// Re-implement SerializeSeq for SeqSer with correct end.
impl<'a, 'b, W: Write> SerializeSeq for SeqSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<()> {
        if self.flow {
            if !self.first { self.ser.out.write_str(", ")?; }
            self.ser.with_in_flow(|s| v.serialize(s))?;
        } else {
            self.ser.write_indent(self.depth)?;
            self.ser.out.write_str("- ")?;
            self.ser.at_line_start = false;
            v.serialize(&mut *self.ser)?;
        }
        self.first = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.flow {
            let mut me = self;
            me.ser.out.write_str("]")?;
            if me.ser.in_flow == 0 { me.ser.newline()?; }
        }
        Ok(())
    }
}

// Tuple-struct serializer (normal or anchor payload)
pub struct TupleSer<'a, 'b, W: Write> {
    ser: &'a mut YamlSer<'b, W>,
    kind: TupleKind,
    idx: usize,
    depth_for_normal: usize,
}
enum TupleKind {
    Normal,       // treat as block seq
    AnchorStrong, // [ptr, value]
    AnchorWeak,   // [ptr, present, value]
}
impl<'a, 'b, W: Write> TupleSer<'a, 'b, W> {
    fn normal(ser: &'a mut YamlSer<'b, W>) -> Self {
        let depth_next = ser.depth + 1;
        Self { ser, kind: TupleKind::Normal, idx: 0, depth_for_normal: depth_next }
    }
    fn anchor_strong(ser: &'a mut YamlSer<'b, W>) -> Self {
        Self { ser, kind: TupleKind::AnchorStrong, idx: 0, depth_for_normal: 0 }
    }
    fn anchor_weak(ser: &'a mut YamlSer<'b, W>) -> Self {
        Self { ser, kind: TupleKind::AnchorWeak, idx: 0, depth_for_normal: 0 }
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
                    if !self.ser.at_line_start { self.ser.newline()?; }
                }
                self.ser.write_indent(self.ser.depth + 1)?;
                self.ser.out.write_str("- ")?;
                self.ser.at_line_start = false;
                value.serialize(&mut *self.ser)?;
            }
            TupleKind::AnchorStrong => {
                match self.idx {
                    0 => {
                        // pointer (usize)
                        let mut cap = UsizeCapture::default();
                        value.serialize(&mut cap)?;
                        let ptr = cap.finish()?;
                        // store ptr for idx 1 via pending_anchor trick:
                        // We'll actually serialize the 2nd field, using ptr then
                        // alias_or_define...
                        self.idx += 1;
                        // consume second value immediately: we expect caller will call us again with value
                        self.idx -= 1;
                        // Define anchor name on first sight without borrowing through closure.
                        if let Some(name) = self.ser.anchors.get(&ptr).cloned() {
                            self.ser.pending_anchor = Some(name);
                        } else {
                            let name = self.ser.new_anchor_name();
                            self.ser.anchors.insert(ptr, name.clone());
                            self.ser.pending_anchor = Some(name);
                        }
                    }
                    1 => {
                        // actual value
                        // We can't retrieve ptr here (we put name already).
                        // Just serialize value; the pending anchor will be emitted.
                        value.serialize(&mut *self.ser)?;
                    }
                    _ => return Err(Error("unexpected field in __yaml_anchor".into())),
                }
            }
            TupleKind::AnchorWeak => {
                match self.idx {
                    0 => {
                        let mut cap = UsizeCapture::default();
                        value.serialize(&mut cap)?;
                        let ptr = cap.finish()?;
                        // stash anchor name if already defined; else prepare to define on first present=true
                        // Put it into pending_anchor only *when* present==true/value arrives.
                        // Save ptr in a side-channel: we'll keep it in UsizeCapture via self.depth_for_normal
                        // (quick hack). Encode ptr into depth_for_normal to avoid extra field.
                        self.depth_for_normal = ptr;
                    }
                    1 => {
                        let mut bc = BoolCapture::default();
                        value.serialize(&mut bc)?;
                        let present = bc.finish()?;
                        if !present {
                            // serialize as null and skip reading the 3rd field's content
                            if self.ser.at_line_start { self.ser.write_indent(self.ser.depth)?; }
                            self.ser.out.write_str("null")?;
                            if self.ser.in_flow == 0 { self.ser.newline()?; }
                        } else {
                            // present => third field carries the node. Define or alias.
                            let ptr = self.depth_for_normal;
                            if self.ser.anchors.contains_key(&ptr) {
                                // already defined; set to alias on third field
                                let name = self.ser.anchors.get(&ptr).cloned().unwrap();
                                // Record name into pending_anchor? No, we want alias, not definition.
                                // We'll emit alias in field #3 directly; mark pending_anchor None.
                                self.ser.pending_anchor = None;
                                // Stash name into depth_for_normal as marker? We'll just keep it:
                                // we can't store String; so do nothing here and handle in field #3.
                                // We'll emit alias there using ptr.
                            } else {
                                // define now
                                let name = self.ser.new_anchor_name();
                                self.ser.anchors.insert(ptr, name.clone());
                                self.ser.pending_anchor = Some(name);
                            }
                        }
                    }
                    2 => {
                        // value of weak reference
                        let ptr = self.depth_for_normal;
                        if let Some(name) = self.ser.anchors.get(&ptr).cloned() {
                            if self.ser.pending_anchor.is_none() {
                                // alias case
                                self.ser.write_alias(&name)?;
                            } else {
                                // definition case: just serialize the value; the pending anchor will be placed.
                                value.serialize(&mut *self.ser)?;
                            }
                        } else {
                            // If we get here with no anchor and no pending, it's a dangling-but-present=false path,
                            // but then we shouldn't have field #3 serialized. Just ignore.
                            // To be safe: serialize unit as null.
                            if self.ser.at_line_start { self.ser.write_indent(self.ser.depth)?; }
                            self.ser.out.write_str("null")?;
                            if self.ser.in_flow == 0 { self.ser.newline()?; }
                        }
                    }
                    _ => return Err(Error("unexpected field in __yaml_weak_anchor".into())),
                }
            }
        }
        self.idx += 1;
        Ok(())
    }

    fn end(self) -> Result<()> { Ok(()) }
}

// Tuple variant (enum Variant: ( ... ))
pub struct TupleVariantSer<'a, 'b, W: Write> {
    ser: &'a mut YamlSer<'b, W>,
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
    fn end(self) -> Result<()> { Ok(()) }
}

// ------------------------------------------------------------
// Map / Struct serializers
// ------------------------------------------------------------

pub struct MapSer<'a, 'b, W: Write> {
    ser: &'a mut YamlSer<'b, W>,
    depth: usize,
    flow: bool,
    first: bool,
}

impl<'a, 'b, W: Write> SerializeMap for MapSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        if self.flow {
            if !self.first { self.ser.out.write_str(", ")?; }
            let text = scalar_key_to_string(key)?;
            self.ser.out.write_str(&text)?;
            self.ser.out.write_str(": ")?;
            self.ser.at_line_start = false;
        } else {
            let text = scalar_key_to_string(key)?;
            self.ser.write_indent(self.depth)?;
            self.ser.out.write_str(&text)?;
            self.ser.out.write_str(": ")?;
            self.ser.at_line_start = false;
        }
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        if self.flow {
            self.ser.with_in_flow(|s| value.serialize(s))?;
        } else {
            value.serialize(&mut *self.ser)?;
        }
        self.first = false;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.flow {
            self.ser.out.write_str("}")?;
            if self.ser.in_flow == 0 { self.ser.newline()?; }
        }
        Ok(())
    }
}
impl<'a, 'b, W: Write> SerializeStruct for MapSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
        SerializeMap::serialize_key(self, &key)?;
        SerializeMap::serialize_value(self, value)
    }
    fn end(self) -> Result<()> { SerializeMap::end(self) }
}

pub struct StructVariantSer<'a, 'b, W: Write> {
    ser: &'a mut YamlSer<'b, W>,
    depth: usize,
}
impl<'a, 'b, W: Write> SerializeStructVariant for StructVariantSer<'a, 'b, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
        let text = scalar_key_to_string(&key)?;
        self.ser.write_indent(self.depth)?;
        self.ser.out.write_str(&text)?;
        self.ser.out.write_str(": ")?;
        self.ser.at_line_start = false;
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<()> { Ok(()) }
}

// ------------------------------------------------------------
// Helpers used for extracting ptr/bool inside tuple payloads
// ------------------------------------------------------------

#[derive(Default)]
struct UsizeCapture { v: Option<usize> }
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

    fn serialize_i8(self, v: i8) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_i16(self, v: i16) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_i32(self, v: i32) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_i64(self, v: i64) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_u8(self, v: u8) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_u16(self, v: u16) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_u32(self, v: u32) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_u64(self, v: u64) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_f32(self, v: f32) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_f64(self, v: f64) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_bool(self, v: bool) -> Result<()> { self.v = Some(v as usize); Ok(()) }
    fn serialize_char(self, _v: char) -> Result<()> { Err(Error("ptr expects number".into())) }
    fn serialize_str (self, _v: &str) -> Result<()> { Err(Error("ptr expects number".into())) }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> { Err(Error("ptr expects number".into())) }
    fn serialize_none(self) -> Result<()> { Err(Error("ptr cannot be none".into())) }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> { Err(Error("ptr not option".into())) }
    fn serialize_unit(self) -> Result<()> { Err(Error("ptr cannot be unit".into())) }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _name: &'static str, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _name: &'static str, _i: u32, _v: &'static str, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple_variant(self, _name: &'static str, _i: u32, _v: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_struct_variant(self, _name: &'static str, _i: u32, _v: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn is_human_readable(&self) -> bool { true }
}
impl UsizeCapture {
    fn finish(self) -> Result<usize> {
        self.v.ok_or_else(|| Error("missing numeric ptr".into()))
    }
}

#[derive(Default)]
struct BoolCapture { v: Option<bool> }
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

    fn serialize_bool(self, v: bool) -> Result<()> { self.v = Some(v); Ok(()) }
    fn serialize_i8(self, _v: i8) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_i16(self, _v: i16) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_i32(self, _v: i32) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_i64(self, _v: i64) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_u8(self, _v: u8) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_u16(self, _v: u16) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_u32(self, _v: u32) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_u64(self, _v: u64) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_f32(self, _v: f32) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_f64(self, _v: f64) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_char(self, _c: char) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_str (self, _v: &str) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_none(self) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_some<T: ?Sized + Serialize>(self, _v: &T) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_unit(self) -> Result<()> { Err(Error("bool expected".into())) }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_unit_variant(self, _name: &'static str, _i: u32, _v: &'static str) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _name: &'static str, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _name: &'static str, _i: u32, _v: &'static str, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_tuple_variant(self, _name: &'static str, _i: u32, _v: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn serialize_struct_variant(self, _name: &'static str, _i: u32, _v: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("unexpected".into())) }
    fn collect_str<T: ?Sized + fmt::Display>(self, _value: &T) -> Result<()> { Err(Error("unexpected".into())) }
    fn is_human_readable(&self) -> bool { true }
}
impl BoolCapture {
    fn finish(self) -> Result<bool> { self.v.ok_or_else(|| Error("missing bool".into())) }
}

// ------------------------------------------------------------
// Key scalar helper
// ------------------------------------------------------------

fn scalar_key_to_string<K: Serialize + ?Sized>(key: &K) -> Result<String> {
    let mut s = String::new();
    {
        let mut ks = KeyScalarSink { s: &mut s };
        key.serialize(&mut ks)?;
    }
    Ok(s)
}

struct KeyScalarSink<'a> { s: &'a mut String }

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
    fn serialize_i64(self, v: i64) -> Result<()> { self.s.push_str(&v.to_string()); Ok(()) }
    fn serialize_i32(self, v: i32) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_i16(self, v: i16) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_i8 (self, v: i8 ) -> Result<()> { self.serialize_i64(v as i64) }
    fn serialize_u64(self, v: u64) -> Result<()> { self.s.push_str(&v.to_string()); Ok(()) }
    fn serialize_u32(self, v: u32) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_u16(self, v: u16) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_u8 (self, v: u8 ) -> Result<()> { self.serialize_u64(v as u64) }
    fn serialize_f32(self, v: f32) -> Result<()> { self.s.push_str(&format!("{}", v)); Ok(()) }
    fn serialize_f64(self, v: f64) -> Result<()> {
        if v.is_nan() { self.s.push_str(".nan"); }
        else if v.is_infinite() { if v.is_sign_positive() { self.s.push_str(".inf"); } else { self.s.push_str("-.inf"); } }
        else { self.s.push_str(&format!("{}", v)); }
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
                    '"'  => self.s.push_str("\\\""),
                    '\n' => self.s.push_str("\\n"),
                    '\r' => self.s.push_str("\\r"),
                    '\t' => self.s.push_str("\\t"),
                    c if c.is_control() => {
                        use std::fmt::Write as _;
                        write!(self.s, "\\u{:04X}", c as u32).unwrap();
                    }
                    c => self.s.push(c),
                }
            }
            self.s.push('"');
        }
        Ok(())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<()> { Err(Error("non-scalar key".into())) }
    fn serialize_none (self) -> Result<()> { self.s.push_str("null"); Ok(()) }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<()> { v.serialize(self) }
    fn serialize_unit(self) -> Result<()> { self.s.push_str("null"); Ok(()) }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> { self.serialize_unit() }
    fn serialize_unit_variant(self, _name: &'static str, _idx: u32, variant: &'static str) -> Result<()> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _name: &'static str, _value: &T) -> Result<()> {
        Err(Error("non-scalar key".into()))
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self,_:&'static str,_:u32,_:&'static str,_:&T)->Result<()>{
        Err(Error("non-scalar key".into()))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("non-scalar key".into())) }
    fn serialize_tuple(self, _len: usize) -> Result<ser::Impossible<(), Error>> { Err(Error("non-scalar key".into())) }
    fn serialize_tuple_struct(self,_:&'static str,_:usize)->Result<ser::Impossible<(), Error>> { Err(Error("non-scalar key".into())) }
    fn serialize_tuple_variant(self,_:&'static str,_:u32,_:&'static str,_:usize)->Result<ser::Impossible<(), Error>>{
        Err(Error("non-scalar key".into()))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<(), Error>> { Err(Error("non-scalar key".into())) }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<ser::Impossible<(), Error>> {
        Err(Error("non-scalar key".into()))
    }
    fn serialize_struct_variant(self,_:&'static str,_:u32,_:&'static str,_:usize)->Result<ser::Impossible<(), Error>>{
        Err(Error("non-scalar key".into()))
    }
    fn collect_str<T: ?Sized + fmt::Display>(self, v: &T) -> Result<()> {
        self.serialize_str(&v.to_string())
    }
    fn is_human_readable(&self) -> bool { true }
}
