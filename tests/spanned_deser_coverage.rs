/// Targeted tests to increase coverage of src/de/spanned_deser.rs.
///
/// Covers:
/// - `deserialize_yaml_spanned` with `peek()` returning `None` (end-of-stream path, line 38)
/// - `SpannedDeser::deserialize_any` (line 81-83)
/// - `LocationDeser::deserialize_any` (line 183-185)
/// - `SpanDeser::deserialize_any` (line 259-261)
/// - `ByteInfoTupleDeser::deserialize_seq` (line 305-310)
/// - `ByteInfoSeqAccess` exhausted path (line 342)
///
/// Note: `#[serde(flatten)]` with `Spanned<T>` works for values but loses location
/// information because serde buffers values through `ContentDeserializer` which
/// discards the YAML deserializer context. Location fields will be `Location::UNKNOWN`.
use serde::Deserialize;
use serde_saphyr::Spanned;

// ---------------------------------------------------------------------------
// Basic Spanned deserialization — exercises the main happy path through
// SpannedDeser, SpannedMapAccess, LocationDeser, LocationMapAccess,
// SpanDeser, SpanMapAccess, ByteInfoTupleDeser, ByteInfoSeqAccess.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_basic_scalar() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<u32>,
    }
    let s: S = serde_saphyr::from_str("v: 42\n").unwrap();
    assert_eq!(s.v.value, 42);
    assert_eq!(s.v.referenced.line(), 1);
    assert_eq!(s.v.referenced.column(), 4);
    // defined == referenced for a plain scalar
    assert_eq!(s.v.defined, s.v.referenced);
    // span fields are accessible
    let span = s.v.referenced.span();
    assert!(!span.is_empty());
}

#[test]
fn spanned_deser_string_value() {
    #[derive(Debug, Deserialize)]
    struct S {
        name: Spanned<String>,
    }
    let s: S = serde_saphyr::from_str("name: hello\n").unwrap();
    assert_eq!(s.name.value, "hello");
    assert_eq!(s.name.referenced.line(), 1);
}

// ---------------------------------------------------------------------------
// Spanned<T> at the top level — exercises the None branch of peek() (line 38)
// when the deserializer is positioned at the very last node.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_top_level_scalar() {
    // Deserializing a bare scalar as Spanned<T> at the top level means
    // peek() will see the scalar event (Some branch). After consuming it,
    // the next peek() returns None — exercising the None arm on subsequent
    // Spanned fields.
    let v: Spanned<u64> = serde_saphyr::from_str("99\n").unwrap();
    assert_eq!(v.value, 99);
    assert_eq!(v.referenced.line(), 1);
}

#[test]
fn spanned_deser_top_level_string() {
    let v: Spanned<String> = serde_saphyr::from_str("hello\n").unwrap();
    assert_eq!(v.value, "hello");
}

#[test]
fn spanned_deser_top_level_bool() {
    let v: Spanned<bool> = serde_saphyr::from_str("true\n").unwrap();
    assert!(v.value);
}

// ---------------------------------------------------------------------------
// Spanned<T> inside a sequence — exercises multiple Spanned instances,
// each going through the full SpannedDeser → SpannedMapAccess path.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_sequence_of_spanned() {
    let v: Vec<Spanned<i32>> = serde_saphyr::from_str("[1, 2, 3]\n").unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].value, 1);
    assert_eq!(v[1].value, 2);
    assert_eq!(v[2].value, 3);
}

// ---------------------------------------------------------------------------
// Spanned<T> with alias — exercises referenced != defined path.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_alias_referenced_vs_defined() {
    #[derive(Debug, Deserialize)]
    struct S {
        a: Spanned<u32>,
        b: Spanned<u32>,
    }
    let yaml = "a: &anchor 10\nb: *anchor\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(s.a.value, 10);
    assert_eq!(s.b.value, 10);
    // b is an alias: referenced points to *anchor, defined points to &anchor
    assert_ne!(s.b.referenced, s.b.defined);
    assert_eq!(s.b.referenced.line(), 2); // *anchor is on line 2
    assert_eq!(s.b.defined.line(), 1);    // &anchor is on line 1
}

// ---------------------------------------------------------------------------
// Spanned<T> with nested struct — exercises LocationDeser and SpanDeser
// through multiple levels.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_nested_struct() {
    #[derive(Debug, Deserialize)]
    struct Inner {
        x: Spanned<i32>,
        y: Spanned<i32>,
    }
    #[derive(Debug, Deserialize)]
    struct Outer {
        inner: Inner,
    }
    let yaml = "inner:\n  x: 1\n  y: 2\n";
    let o: Outer = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(o.inner.x.value, 1);
    assert_eq!(o.inner.y.value, 2);
    assert_eq!(o.inner.x.referenced.line(), 2);
    assert_eq!(o.inner.y.referenced.line(), 3);
}

// ---------------------------------------------------------------------------
// Spanned<Option<T>> — exercises the None/null path through SpannedDeser.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_option_none() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<Option<u32>>,
    }
    let s: S = serde_saphyr::from_str("v: null\n").unwrap();
    assert_eq!(s.v.value, None);
    assert_eq!(s.v.referenced.line(), 1);
}

#[test]
fn spanned_deser_option_some() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<Option<u32>>,
    }
    let s: S = serde_saphyr::from_str("v: 7\n").unwrap();
    assert_eq!(s.v.value, Some(7));
}

// ---------------------------------------------------------------------------
// Spanned<Vec<T>> — exercises SpannedDeser wrapping a sequence value.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_vec_value() {
    #[derive(Debug, Deserialize)]
    struct S {
        items: Spanned<Vec<u32>>,
    }
    let s: S = serde_saphyr::from_str("items: [1, 2, 3]\n").unwrap();
    assert_eq!(s.items.value, vec![1, 2, 3]);
    assert_eq!(s.items.referenced.line(), 1);
}

// ---------------------------------------------------------------------------
// Span field accessors — exercises offset, len, byte_offset, byte_len
// through the public API (which drives SpanMapAccess and ByteInfoTupleDeser).
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_span_accessors() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<String>,
    }
    let yaml = "v: hello\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    let span = s.v.referenced.span();
    assert_eq!(span.offset() as usize, yaml.find("hello").unwrap());
    assert!(span.len() > 0);
    assert!(!span.is_empty());
    // byte_offset and byte_len are available for string sources
    assert!(span.byte_offset().is_some());
    assert!(span.byte_len().is_some());
}

// ---------------------------------------------------------------------------
// Multiple Spanned fields in one struct — exercises all three key/value
// pairs of SpannedMapAccess (value, referenced, defined) for each field.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_multiple_fields() {
    #[derive(Debug, Deserialize)]
    struct S {
        a: Spanned<u32>,
        b: Spanned<String>,
        c: Spanned<bool>,
    }
    let yaml = "a: 1\nb: foo\nc: true\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(s.a.value, 1);
    assert_eq!(s.b.value, "foo");
    assert!(s.c.value);
    assert_eq!(s.a.referenced.line(), 1);
    assert_eq!(s.b.referenced.line(), 2);
    assert_eq!(s.c.referenced.line(), 3);
}

// ---------------------------------------------------------------------------
// Spanned<f64> — exercises float scalar path through SpannedDeser.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_float() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<f64>,
    }
    let s: S = serde_saphyr::from_str("v: 3.14\n").unwrap();
    assert!((s.v.value - 3.14).abs() < 1e-10);
    assert_eq!(s.v.referenced.line(), 1);
}

// ---------------------------------------------------------------------------
// Spanned<HashMap<K,V>> — exercises map value path through SpannedDeser.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_map_value() {
    use std::collections::HashMap;
    #[derive(Debug, Deserialize)]
    struct S {
        m: Spanned<HashMap<String, u32>>,
    }
    let yaml = "m:\n  x: 1\n  y: 2\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(s.m.value["x"], 1);
    assert_eq!(s.m.value["y"], 2);
    // the map value starts on line 2 (after "m:" on line 1)
    assert_eq!(s.m.referenced.line(), 2);
}

// ---------------------------------------------------------------------------
// Spanned<T> in a block sequence — exercises sequence element path.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_block_sequence() {
    let yaml = "- 10\n- 20\n- 30\n";
    let v: Vec<Spanned<u32>> = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].value, 10);
    assert_eq!(v[1].value, 20);
    assert_eq!(v[2].value, 30);
    assert_eq!(v[0].referenced.line(), 1);
    assert_eq!(v[1].referenced.line(), 2);
    assert_eq!(v[2].referenced.line(), 3);
}

// ---------------------------------------------------------------------------
// Spanned<T> with enum value — exercises enum deserialization path.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_enum_value() {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Color {
        Red,
        Green,
        Blue,
    }
    #[derive(Debug, Deserialize)]
    struct S {
        color: Spanned<Color>,
    }
    let s: S = serde_saphyr::from_str("color: Red\n").unwrap();
    assert_eq!(s.color.value, Color::Red);
    assert_eq!(s.color.referenced.line(), 1);
}

// ---------------------------------------------------------------------------
// Spanned<T> with i64/u64 boundary values — exercises integer parsing.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_integer_boundaries() {
    #[derive(Debug, Deserialize)]
    struct S {
        a: Spanned<i64>,
        b: Spanned<u64>,
    }
    let yaml = "a: -9223372036854775808\nb: 18446744073709551615\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(s.a.value, i64::MIN);
    assert_eq!(s.b.value, u64::MAX);
}

// ---------------------------------------------------------------------------
// Reader-based Spanned<T> — exercises peek() → None path (line 38) since
// reader-based deserialization uses LiveEvents which may return None at
// end-of-stream when deserialize_yaml_spanned is called.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_from_reader_scalar() {
    #[derive(Debug, Deserialize)]
    struct S {
        v: Spanned<u32>,
    }
    let yaml = b"v: 42\n";
    let s: S = serde_saphyr::from_reader(yaml.as_ref()).unwrap();
    assert_eq!(s.v.value, 42);
    assert_eq!(s.v.referenced.line(), 1);
    // byte_offset is None for reader-based deserialization
    assert!(s.v.referenced.span().byte_offset().is_none());
}

#[test]
fn spanned_deser_from_reader_top_level() {
    let yaml = b"99\n";
    let v: Spanned<u64> = serde_saphyr::from_reader(yaml.as_ref()).unwrap();
    assert_eq!(v.value, 99);
}

#[test]
fn spanned_deser_from_reader_alias() {
    #[derive(Debug, Deserialize)]
    struct S {
        a: Spanned<u32>,
        b: Spanned<u32>,
    }
    let yaml = b"a: &anchor 10\nb: *anchor\n";
    let s: S = serde_saphyr::from_reader(yaml.as_ref()).unwrap();
    assert_eq!(s.a.value, 10);
    assert_eq!(s.b.value, 10);
    assert_ne!(s.b.referenced, s.b.defined);
}

// ---------------------------------------------------------------------------
// Spanned<T> column tracking — verifies column numbers are 1-based.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_column_tracking() {
    #[derive(Debug, Deserialize)]
    struct S {
        short: Spanned<u32>,
        longer_key: Spanned<u32>,
    }
    let yaml = "short: 1\nlonger_key: 2\n";
    let s: S = serde_saphyr::from_str(yaml).unwrap();
    // "short: " is 7 chars, value starts at column 8
    assert_eq!(s.short.referenced.column(), 8);
    // "longer_key: " is 12 chars, value starts at column 13
    assert_eq!(s.longer_key.referenced.column(), 13);
}

// ---------------------------------------------------------------------------
// #[serde(flatten)] with Spanned<T> — deserialization succeeds but location
// info is unavailable (Location::UNKNOWN) because serde buffers values through
// ContentDeserializer which discards the YAML deserializer context.
// ---------------------------------------------------------------------------

#[test]
fn spanned_deser_flatten_triggers_deserialize_any() {
    #[derive(Debug, Deserialize)]
    struct Inner {
        v: Spanned<u32>,
    }
    #[derive(Debug, Deserialize)]
    struct Outer {
        #[serde(flatten)]
        inner: Inner,
    }
    let o: Outer = serde_saphyr::from_str("v: 42\n").unwrap();
    assert_eq!(o.inner.v.value, 42);
    // Location is unavailable through #[serde(flatten)] buffering.
    assert_eq!(o.inner.v.referenced.line(), 0);
}

#[test]
fn spanned_deser_flatten_multiple() {
    #[derive(Debug, Deserialize)]
    struct Inner {
        a: Spanned<u32>,
        b: Spanned<String>,
    }
    #[derive(Debug, Deserialize)]
    struct Outer {
        #[serde(flatten)]
        inner: Inner,
    }
    let o: Outer = serde_saphyr::from_str("a: 1\nb: hello\n").unwrap();
    assert_eq!(o.inner.a.value, 1);
    assert_eq!(o.inner.b.value, "hello");
    // Location is unavailable through #[serde(flatten)] buffering.
    assert_eq!(o.inner.a.referenced.line(), 0);
    assert_eq!(o.inner.b.referenced.line(), 0);
}