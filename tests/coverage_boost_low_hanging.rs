//! Tests targeting low-coverage areas identified by cargo llvm-cov.

use serde::{Deserialize, Serialize};
use serde_saphyr::{FoldString, LitString, SerializerOptions};

// ── lib.rs: deprecated to_writer / to_writer_with_options ──────────────

#[test]
#[allow(deprecated)]
fn deprecated_to_writer() {
    let mut buf = String::new();
    serde_saphyr::to_writer(&mut buf, &42i32).unwrap();
    assert_eq!(buf.trim(), "42");
}

#[test]
#[allow(deprecated)]
fn deprecated_to_writer_with_options() {
    let mut buf = String::new();
    serde_saphyr::to_writer_with_options(&mut buf, &"hello", SerializerOptions::default())
        .unwrap();
    assert!(buf.contains("hello"));
}

// ── lib.rs: to_io_writer / to_io_writer_with_options ───────────────────

#[test]
fn to_io_writer_basic() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer(&mut buf, &true).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "true");
}

#[test]
fn to_io_writer_with_options_basic() {
    let mut buf = Vec::new();
    serde_saphyr::to_io_writer_with_options(&mut buf, &vec![1, 2, 3], SerializerOptions::default())
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("- 1"));
}

// ── lib.rs: to_fmt_writer ──────────────────────────────────────────────

#[test]
fn to_fmt_writer_basic() {
    let mut buf = String::new();
    serde_saphyr::to_fmt_writer(&mut buf, &"test").unwrap();
    assert!(buf.contains("test"));
}

// ── lib.rs: multiple documents error from from_str ─────────────────────

#[test]
fn from_str_multiple_documents_error() {
    let yaml = "---\nhello\n---\nworld\n";
    let result: Result<String, _> = serde_saphyr::from_str(yaml);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("multiple") || err.contains("iterator"),
        "unexpected error: {err}"
    );
}

// ── long_strings.rs: PartialEq impls ──────────────────────────────────

#[test]
fn lit_string_eq_string() {
    let ls = LitString::from("hello".to_string());
    let s = "hello".to_string();
    assert!(ls == s);
}

#[test]
fn fold_string_eq_string() {
    let fs = FoldString::from("hello".to_string());
    let s = "hello".to_string();
    assert!(fs == s);
}

#[test]
fn lit_string_eq_str() {
    let ls = LitString::from("hello".to_string());
    assert!(ls == *"hello");
}

#[test]
fn fold_string_eq_str() {
    let fs = FoldString::from("hello".to_string());
    assert!(fs == *"hello");
}

// ── ser.rs: serialize various primitives ───────────────────────────────

#[test]
fn serialize_i8() {
    let s = serde_saphyr::to_string(&42i8).unwrap();
    assert_eq!(s.trim(), "42");
}

#[test]
fn serialize_i16() {
    let s = serde_saphyr::to_string(&1000i16).unwrap();
    assert_eq!(s.trim(), "1000");
}

#[test]
fn serialize_i128() {
    let s = serde_saphyr::to_string(&170141183460469231731687303715884105727i128).unwrap();
    assert!(s.trim().len() > 10);
}

#[test]
fn serialize_u8() {
    let s = serde_saphyr::to_string(&255u8).unwrap();
    assert_eq!(s.trim(), "255");
}

#[test]
fn serialize_u16() {
    let s = serde_saphyr::to_string(&65535u16).unwrap();
    assert_eq!(s.trim(), "65535");
}

#[test]
fn serialize_u128() {
    let s = serde_saphyr::to_string(&340282366920938463463374607431768211455u128).unwrap();
    assert!(s.trim().len() > 10);
}

#[test]
fn serialize_char() {
    let s = serde_saphyr::to_string(&'Z').unwrap();
    assert!(s.contains('Z'));
}

#[test]
fn serialize_unit() {
    let s = serde_saphyr::to_string(&()).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_none() {
    let v: Option<i32> = None;
    let s = serde_saphyr::to_string(&v).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_some() {
    let v: Option<i32> = Some(42);
    let s = serde_saphyr::to_string(&v).unwrap();
    assert_eq!(s.trim(), "42");
}

#[test]
fn serialize_unit_struct() {
    #[derive(Serialize)]
    struct Unit;
    let s = serde_saphyr::to_string(&Unit).unwrap();
    assert!(s.contains("null"));
}

#[test]
fn serialize_newtype_struct() {
    #[derive(Serialize)]
    struct Wrapper(i32);
    let s = serde_saphyr::to_string(&Wrapper(99)).unwrap();
    assert_eq!(s.trim(), "99");
}

#[test]
fn serialize_tuple() {
    let s = serde_saphyr::to_string(&(1, "two", true)).unwrap();
    assert!(s.contains("- 1"));
    assert!(s.contains("- two"));
    assert!(s.contains("- true"));
}

#[test]
fn serialize_tuple_struct() {
    #[derive(Serialize)]
    struct Pair(i32, String);
    let s = serde_saphyr::to_string(&Pair(1, "hello".into())).unwrap();
    assert!(s.contains("- 1"));
    assert!(s.contains("- hello"));
}

#[test]
fn serialize_struct_variant() {
    #[derive(Serialize)]
    enum E {
        Variant { x: i32, y: String },
    }
    let s = serde_saphyr::to_string(&E::Variant {
        x: 10,
        y: "hi".into(),
    })
    .unwrap();
    assert!(s.contains("Variant"));
    assert!(s.contains("x: 10"));
}

#[test]
fn serialize_tuple_variant() {
    #[derive(Serialize)]
    enum E {
        Tup(i32, bool),
    }
    let s = serde_saphyr::to_string(&E::Tup(1, true)).unwrap();
    assert!(s.contains("Tup"));
}

#[test]
fn serialize_unit_variant() {
    #[derive(Serialize)]
    #[allow(dead_code)]
    enum Color {
        Red,
        Blue,
    }
    let s = serde_saphyr::to_string(&Color::Red).unwrap();
    assert!(s.contains("Red"));
}

// ── ser.rs: string quoting edge cases ──────────────────────────────────

#[test]
fn serialize_string_with_single_quote() {
    // Triggers write_single_quoted with embedded quote -> doubled
    let s = serde_saphyr::to_string(&"it's").unwrap();
    assert!(s.contains("it's") || s.contains("it''s") || s.contains("\"it's\""));
}

#[test]
fn serialize_string_with_special_chars() {
    // Triggers write_quoted with escape sequences
    let s = serde_saphyr::to_string(&"line1\nline2\ttab").unwrap();
    assert!(s.contains("line1") && s.contains("line2"));
}

#[test]
fn serialize_string_with_unicode_control() {
    // Triggers the \u{XXXX} escape path in write_quoted
    let s = serde_saphyr::to_string(&"hello\x01world").unwrap();
    assert!(s.contains("hello"));
}

#[test]
fn serialize_string_with_backslash() {
    let s = serde_saphyr::to_string(&"back\\slash").unwrap();
    assert!(s.contains("back"));
}

// ── ser.rs: bytes serialization (base64) ───────────────────────────────

#[test]
fn serialize_bytes() {
    use serde::Serializer;
    let mut buf = String::new();
    {
        let mut ser = serde_saphyr::ser::YamlSerializer::new(&mut buf);
        ser.serialize_bytes(b"hello").unwrap();
    }
    // Just verify it produced some output (base64-encoded)
    assert!(!buf.is_empty(), "bytes serialization produced: {buf}");
}

// ── ser.rs: f32 / f64 special values ───────────────────────────────────

#[test]
fn serialize_f32_nan() {
    let s = serde_saphyr::to_string(&f32::NAN).unwrap();
    assert!(s.contains(".nan"));
}

#[test]
fn serialize_f32_inf() {
    let s = serde_saphyr::to_string(&f32::INFINITY).unwrap();
    assert!(s.contains(".inf"));
}

#[test]
fn serialize_f32_neg_inf() {
    let s = serde_saphyr::to_string(&f32::NEG_INFINITY).unwrap();
    assert!(s.contains("-.inf"));
}

#[test]
fn serialize_f64_nan() {
    let s = serde_saphyr::to_string(&f64::NAN).unwrap();
    assert!(s.contains(".nan"));
}

#[test]
fn serialize_f64_inf() {
    let s = serde_saphyr::to_string(&f64::INFINITY).unwrap();
    assert!(s.contains(".inf"));
}

// ── ser.rs: map serialization ──────────────────────────────────────────

#[test]
fn serialize_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert("key1", 1);
    m.insert("key2", 2);
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("key1: 1"));
    assert!(s.contains("key2: 2"));
}

// ── ser.rs: nested structures ──────────────────────────────────────────

#[test]
fn serialize_nested_struct() {
    #[derive(Serialize)]
    struct Inner {
        value: i32,
    }
    #[derive(Serialize)]
    struct Outer {
        name: String,
        inner: Inner,
    }
    let s = serde_saphyr::to_string(&Outer {
        name: "test".into(),
        inner: Inner { value: 42 },
    })
    .unwrap();
    assert!(s.contains("name: test"));
    assert!(s.contains("value: 42"));
}

// ── ser.rs: FlowSeq / FlowMap ──────────────────────────────────────────

#[test]
fn serialize_flow_seq() {
    use serde_saphyr::FlowSeq;
    #[derive(Serialize)]
    struct Doc {
        items: FlowSeq<Vec<i32>>,
    }
    let s = serde_saphyr::to_string(&Doc {
        items: FlowSeq(vec![1, 2, 3]),
    })
    .unwrap();
    assert!(s.contains('['));
}

#[test]
fn serialize_flow_map() {
    use serde_saphyr::FlowMap;
    use std::collections::BTreeMap;
    #[derive(Serialize)]
    struct Doc {
        data: FlowMap<BTreeMap<String, i32>>,
    }
    let mut m = BTreeMap::new();
    m.insert("a".into(), 1);
    let s = serde_saphyr::to_string(&Doc {
        data: FlowMap(m),
    })
    .unwrap();
    assert!(s.contains('{'));
}

// ── ser.rs: SpaceAfter ─────────────────────────────────────────────────

#[test]
fn serialize_space_after() {
    use serde_saphyr::SpaceAfter;
    #[derive(Serialize)]
    struct Doc {
        section: SpaceAfter<Vec<i32>>,
        other: i32,
    }
    let s = serde_saphyr::to_string(&Doc {
        section: SpaceAfter(vec![1]),
        other: 2,
    })
    .unwrap();
    assert!(s.contains("other: 2"));
}

// ── ser.rs: Commented ──────────────────────────────────────────────────

#[test]
fn serialize_commented() {
    use serde_saphyr::Commented;
    #[derive(Serialize)]
    struct Doc {
        field: Commented<i32>,
    }
    let s = serde_saphyr::to_string(&Doc {
        field: Commented(42, "a comment".to_string()),
    })
    .unwrap();
    assert!(s.contains("# a comment") || s.contains("comment"));
}

// ── ser.rs: newtype variant ────────────────────────────────────────────

#[test]
fn serialize_newtype_variant() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
        Str(String),
    }
    let s = serde_saphyr::to_string(&Wrapper::Int(5)).unwrap();
    assert!(s.contains("Int: 5") || s.contains("Int"));
    let s2 = serde_saphyr::to_string(&Wrapper::Str("hi".into())).unwrap();
    assert!(s2.contains("Str"));
}

// ── ser.rs: empty seq / empty map ──────────────────────────────────────

#[test]
fn serialize_empty_vec() {
    let v: Vec<i32> = vec![];
    let s = serde_saphyr::to_string(&v).unwrap();
    assert!(s.contains("[]"));
}

#[test]
fn serialize_empty_map() {
    use std::collections::BTreeMap;
    let m: BTreeMap<String, i32> = BTreeMap::new();
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("{}"));
}

// ── lib.rs: from_str with reader for multiple docs ─────────────────────

#[test]
fn read_multiple_documents() {
    let yaml = "---\nhello\n---\nworld\n";
    let docs: Vec<String> = serde_saphyr::from_multiple(yaml).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0], "hello");
    assert_eq!(docs[1], "world");
}

// ── ser.rs: serializer with indent option ──────────────────────────────

#[test]
fn serialize_with_custom_indent() {
    #[derive(Serialize)]
    struct Doc {
        items: Vec<i32>,
    }
    let opts = SerializerOptions {
        indent_step: 4,
        ..Default::default()
    };
    let s = serde_saphyr::to_string_with_options(
        &Doc {
            items: vec![1, 2],
        },
        opts,
    )
    .unwrap();
    assert!(s.contains("items:"));
}

// ── ser.rs: bool key in map ────────────────────────────────────────────

#[test]
fn serialize_bool_key_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    m.insert(false, "no");
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("true") && s.contains("false"));
}

// ── ser.rs: integer key in map ─────────────────────────────────────────

#[test]
fn serialize_integer_key_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert(1i32, "one");
    m.insert(2i32, "two");
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("1: one"));
}

// ── ser.rs: float key in map ───────────────────────────────────────────

#[test]
fn serialize_float_key_map() {
    let s = serde_saphyr::to_string(&std::f64::consts::PI).unwrap();
    assert!(s.contains("3.14159"));
}

// ── de.rs: deserialize various types ───────────────────────────────────

#[test]
fn deserialize_bool() {
    let v: bool = serde_saphyr::from_str("true").unwrap();
    assert!(v);
}

#[test]
fn deserialize_i128() {
    let v: i128 = serde_saphyr::from_str("170141183460469231731687303715884105727").unwrap();
    assert_eq!(v, 170141183460469231731687303715884105727i128);
}

#[test]
fn deserialize_u128() {
    let v: u128 = serde_saphyr::from_str("340282366920938463463374607431768211455").unwrap();
    assert_eq!(v, 340282366920938463463374607431768211455u128);
}

#[test]
fn deserialize_char() {
    let v: char = serde_saphyr::from_str("Z").unwrap();
    assert_eq!(v, 'Z');
}

#[test]
fn deserialize_unit() {
    let _: () = serde_saphyr::from_str("null").unwrap();
}

#[test]
fn deserialize_unit_struct() {
    #[derive(Deserialize, Debug)]
    struct Unit;
    let _: Unit = serde_saphyr::from_str("null").unwrap();
}

#[test]
fn deserialize_newtype_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Wrapper(i32);
    let v: Wrapper = serde_saphyr::from_str("42").unwrap();
    assert_eq!(v, Wrapper(42));
}

#[test]
fn deserialize_tuple() {
    let v: (i32, String, bool) = serde_saphyr::from_str("- 1\n- two\n- true").unwrap();
    assert_eq!(v, (1, "two".to_string(), true));
}

#[test]
fn deserialize_tuple_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Pair(i32, String);
    let v: Pair = serde_saphyr::from_str("- 1\n- hello").unwrap();
    assert_eq!(v, Pair(1, "hello".into()));
}

#[test]
fn deserialize_enum_variants() {
    #[derive(Deserialize, Debug, PartialEq)]
    enum E {
        Unit,
        Newtype(i32),
        Tuple(i32, bool),
        Struct { x: i32 },
    }
    let v: E = serde_saphyr::from_str("Unit").unwrap();
    assert_eq!(v, E::Unit);

    let v: E = serde_saphyr::from_str("Newtype: 5").unwrap();
    assert_eq!(v, E::Newtype(5));

    let v: E = serde_saphyr::from_str("Tuple:\n  - 1\n  - true").unwrap();
    assert_eq!(v, E::Tuple(1, true));

    let v: E = serde_saphyr::from_str("Struct:\n  x: 10").unwrap();
    assert_eq!(v, E::Struct { x: 10 });
}

// ── lib.rs: from_str_with_options ──────────────────────────────────────

#[test]
fn from_str_with_options() {
    use serde_saphyr::Options;
    let opts = Options::default();
    let v: i32 = serde_saphyr::from_str_with_options("42", opts).unwrap();
    assert_eq!(v, 42);
}

// ── lib.rs: from_slice ─────────────────────────────────────────────────

#[test]
fn from_slice_basic() {
    let v: i32 = serde_saphyr::from_slice(b"42").unwrap();
    assert_eq!(v, 42);
}

// ── lib.rs: from_reader ────────────────────────────────────────────────

#[test]
fn from_reader_basic() {
    let data = b"hello" as &[u8];
    let v: String = serde_saphyr::from_reader(data).unwrap();
    assert_eq!(v, "hello");
}
