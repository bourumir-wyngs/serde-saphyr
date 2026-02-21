//! Tests to increase code coverage for `spanned.rs`.
//!
//! The fallback `visit_*` methods in `ReprOrPlainVisitor` are exercised when
//! `Spanned<T>` appears inside a `#[serde(flatten)]` struct, because serde's
//! `ContentDeserializer` dispatches plain scalars directly to the visitor.

use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use serde_saphyr::Spanned;

// ── Helper: flatten wrapper ──────────────────────────────────────────────────

macro_rules! flat_struct {
    ($name:ident, $ty:ty) => {
        #[derive(Debug, Deserialize)]
        struct $name {
            #[serde(flatten)]
            inner: std::collections::HashMap<String, Spanned<$ty>>,
        }
    };
}

flat_struct!(FlatBool, bool);
flat_struct!(FlatI32, i32);
flat_struct!(FlatI64, i64);
flat_struct!(FlatU32, u32);
flat_struct!(FlatF32, f32);
flat_struct!(FlatF64, f64);
flat_struct!(FlatUnit, ());
flat_struct!(FlatOption, Option<i32>);
flat_struct!(FlatVec, Vec<i32>);

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn flatten_spanned_bool() {
    let v: FlatBool = serde_saphyr::from_str("val: true\n").unwrap();
    assert_eq!(v.inner["val"].value, true);
}

#[test]
fn flatten_spanned_i32() {
    let v: FlatI32 = serde_saphyr::from_str("val: -42\n").unwrap();
    assert_eq!(v.inner["val"].value, -42);
}

#[test]
fn flatten_spanned_i64() {
    let v: FlatI64 = serde_saphyr::from_str("val: -9999999999\n").unwrap();
    assert_eq!(v.inner["val"].value, -9_999_999_999i64);
}

#[test]
fn flatten_spanned_u32() {
    let v: FlatU32 = serde_saphyr::from_str("val: 42\n").unwrap();
    assert_eq!(v.inner["val"].value, 42);
}

#[test]
fn flatten_spanned_f32() {
    let v: FlatF32 = serde_saphyr::from_str("val: 3.14\n").unwrap();
    assert!((v.inner["val"].value - 3.14).abs() < 0.01);
}

#[test]
fn flatten_spanned_f64() {
    let v: FlatF64 = serde_saphyr::from_str("val: 2.718\n").unwrap();
    assert!((v.inner["val"].value - 2.718).abs() < 0.001);
}

#[test]
fn flatten_spanned_unit() {
    let v: FlatUnit = serde_saphyr::from_str("val: ~\n").unwrap();
    assert_eq!(v.inner["val"].value, ());
}

#[test]
fn flatten_spanned_none() {
    let v: FlatOption = serde_saphyr::from_str("val: ~\n").unwrap();
    assert_eq!(v.inner["val"].value, None);
}

#[test]
fn flatten_spanned_seq() {
    let v: FlatVec = serde_saphyr::from_str("val:\n  - 1\n  - 2\n  - 3\n").unwrap();
    assert_eq!(v.inner["val"].value, vec![1, 2, 3]);
}

// ── Custom deserializer to exercise individual visit_* fallback methods ──────
//
// Spanned<T>::deserialize calls deserialize_newtype_struct("__yaml_spanned", SpannedVisitor).
// SpannedVisitor::visit_newtype_struct calls deserializer.deserialize_any(ReprOrPlainVisitor).
// We build a custom deserializer that, for deserialize_any, calls the specific visit_* we want.

/// A deserializer whose `deserialize_any` calls a specific `visit_*` method.
/// For `deserialize_newtype_struct`, it delegates to `visitor.visit_newtype_struct(self)`.
#[derive(Clone)]
enum ScalarDeser {
    I8(i8),
    I16(i16),
    I32(i32),
    U8(u8),
    U16(u16),
    U32(u32),
    F32(f32),
    Char(char),
    Bytes(Vec<u8>),
    ByteBuf(Vec<u8>),
    None,
}

impl<'de> Deserializer<'de> for ScalarDeser {
    type Error = serde::de::value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            ScalarDeser::I8(v) => visitor.visit_i8(v),
            ScalarDeser::I16(v) => visitor.visit_i16(v),
            ScalarDeser::I32(v) => visitor.visit_i32(v),
            ScalarDeser::U8(v) => visitor.visit_u8(v),
            ScalarDeser::U16(v) => visitor.visit_u16(v),
            ScalarDeser::U32(v) => visitor.visit_u32(v),
            ScalarDeser::F32(v) => visitor.visit_f32(v),
            ScalarDeser::Char(v) => visitor.visit_char(v),
            ScalarDeser::Bytes(v) => visitor.visit_bytes(&v),
            ScalarDeser::ByteBuf(v) => visitor.visit_byte_buf(v),
            ScalarDeser::None => visitor.visit_none(),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

#[test]
fn visit_i8_fallback() {
    let s: Spanned<i8> = Spanned::deserialize(ScalarDeser::I8(-3)).unwrap();
    assert_eq!(s.value, -3);
}

#[test]
fn visit_i16_fallback() {
    let s: Spanned<i16> = Spanned::deserialize(ScalarDeser::I16(-300)).unwrap();
    assert_eq!(s.value, -300);
}

#[test]
fn visit_i32_fallback() {
    let s: Spanned<i32> = Spanned::deserialize(ScalarDeser::I32(-70000)).unwrap();
    assert_eq!(s.value, -70000);
}

#[test]
fn visit_u8_fallback() {
    let s: Spanned<u8> = Spanned::deserialize(ScalarDeser::U8(200)).unwrap();
    assert_eq!(s.value, 200);
}

#[test]
fn visit_u16_fallback() {
    let s: Spanned<u16> = Spanned::deserialize(ScalarDeser::U16(60000)).unwrap();
    assert_eq!(s.value, 60000);
}

#[test]
fn visit_u32_fallback() {
    let s: Spanned<u32> = Spanned::deserialize(ScalarDeser::U32(100_000)).unwrap();
    assert_eq!(s.value, 100_000);
}

#[test]
fn visit_f32_fallback() {
    let s: Spanned<f32> = Spanned::deserialize(ScalarDeser::F32(1.5)).unwrap();
    assert!((s.value - 1.5).abs() < f32::EPSILON);
}

#[test]
fn visit_char_fallback() {
    let s: Spanned<char> = Spanned::deserialize(ScalarDeser::Char('Z')).unwrap();
    assert_eq!(s.value, 'Z');
}

#[test]
fn visit_none_fallback() {
    let s: Spanned<()> = Spanned::deserialize(ScalarDeser::None).unwrap();
    assert_eq!(s.value, ());
}

// ── visit_bytes / visit_byte_buf fallbacks ───────────────────────────────────

/// A type that deserializes from bytes.
#[derive(Debug, PartialEq)]
struct ByteVec(Vec<u8>);

impl<'de> Deserialize<'de> for ByteVec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ByteVec;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bytes")
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(ByteVec(v.to_vec()))
            }
            fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(ByteVec(v))
            }
        }
        deserializer.deserialize_bytes(V)
    }
}

#[test]
fn visit_bytes_fallback() {
    let s: Spanned<ByteVec> = Spanned::deserialize(ScalarDeser::Bytes(vec![1, 2, 3])).unwrap();
    assert_eq!(s.value, ByteVec(vec![1, 2, 3]));
}

#[test]
fn visit_byte_buf_fallback() {
    let s: Spanned<ByteVec> = Spanned::deserialize(ScalarDeser::ByteBuf(vec![4, 5])).unwrap();
    assert_eq!(s.value, ByteVec(vec![4, 5]));
}

// ── expecting coverage via type mismatch ─────────────────────────────────────

/// A deserializer that calls `deserialize_any` → `visit_enum` which is NOT
/// implemented by ReprOrPlainVisitor, triggering the `expecting` error message.
struct BadDeser;

impl<'de> Deserializer<'de> for BadDeser {
    type Error = serde::de::value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // visit_enum is not implemented by ReprOrPlainVisitor, so this will
        // call expecting() to produce an error message.
        visitor.visit_enum(serde::de::value::MapAccessDeserializer::new(
            serde::de::value::MapDeserializer::new(std::iter::empty::<(String, String)>()),
        ))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

#[test]
fn expecting_is_called_on_type_mismatch() {
    let result = Spanned::<String>::deserialize(BadDeser);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("value or a span-aware map"),
        "expected ReprOrPlainVisitor expecting message, got: {msg}"
    );
}

/// A deserializer that calls visit_map on the SpannedVisitor (outer), which
/// doesn't implement visit_map, triggering its `expecting` method.
struct BadOuterDeser;

impl<'de> Deserializer<'de> for BadOuterDeser {
    type Error = serde::de::value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        unreachable!()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        // Call visit_map instead of visit_newtype_struct — SpannedVisitor doesn't
        // implement visit_map, so serde's default will call expecting().
        visitor.visit_map(serde::de::value::MapDeserializer::new(
            std::iter::empty::<(String, String)>(),
        ))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

#[test]
fn spanned_visitor_expecting_is_called() {
    let result = Spanned::<String>::deserialize(BadOuterDeser);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("span-aware newtype wrapper"),
        "expected SpannedVisitor expecting message, got: {msg}"
    );
}

// ── Serialization round-trip ─────────────────────────────────────────────────

#[test]
fn spanned_serialize_roundtrip() {
    use serde::Serialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Cfg {
        timeout: Spanned<u64>,
    }

    let cfg: Cfg = serde_saphyr::from_str("timeout: 5\n").unwrap();
    let yaml = serde_saphyr::to_string(&cfg).unwrap();
    assert_eq!(yaml, "timeout: 5\n");
}

// ── Spanned::new constructor ─────────────────────────────────────────────────

#[test]
fn spanned_new_constructor() {
    let s = Spanned::new(42u32, serde_saphyr::Location::UNKNOWN, serde_saphyr::Location::UNKNOWN);
    assert_eq!(s.value, 42);
}

// ── Clone + Debug + PartialEq ────────────────────────────────────────────────

#[test]
fn spanned_clone_debug_eq() {
    let a = Spanned::new("hello".to_string(), serde_saphyr::Location::UNKNOWN, serde_saphyr::Location::UNKNOWN);
    let b = a.clone();
    assert_eq!(a, b);
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("hello"));
}
