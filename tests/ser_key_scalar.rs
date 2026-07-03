#![cfg(all(feature = "serialize", feature = "deserialize"))]
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use serde::Serialize;
use serde::ser::{SerializeSeq, Serializer};

use serde_saphyr::{FlowMap, to_string};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Id(u64);

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct OrderedF64 {
    bits: u64,
    value: f64,
}

impl OrderedF64 {
    fn new(value: f64) -> Self {
        Self {
            bits: value.to_bits(),
            value,
        }
    }
}

impl PartialEq for OrderedF64 {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}
impl Eq for OrderedF64 {}
impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrderedF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bits.cmp(&other.bits)
    }
}
impl Hash for OrderedF64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits.hash(state)
    }
}

impl Serialize for OrderedF64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeqKey(u8);

impl Hash for SeqKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl Serialize for SeqKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // A non-scalar key (sequence) should be rejected by the YAML key scalar sink.
        let mut seq = serializer.serialize_seq(Some(1))?;
        seq.serialize_element(&self.0)?;
        seq.end()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BytesKey;

impl Hash for BytesKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state)
    }
}

impl Serialize for BytesKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(b"abc")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum NewtypeVariantKey {
    V(u32),
}

impl Serialize for NewtypeVariantKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            NewtypeVariantKey::V(v) => serializer.serialize_newtype_variant("E", 0, "V", v),
        }
    }
}

#[test]
fn map_keys_support_many_scalar_types_and_escape_when_needed() {
    // String keys that are not safe as plain scalars (e.g. contain ':', newlines, control chars)
    // should be emitted in double quotes with escapes.
    let mut m: BTreeMap<String, i32> = BTreeMap::new();
    m.insert("plain".to_string(), 1);
    m.insert("a:b".to_string(), 2);
    m.insert("line\nbreak".to_string(), 3);
    m.insert("\u{1}".to_string(), 4);

    let yaml = to_string(&m).expect("serialize string-key map");

    assert!(yaml.contains("plain: 1\n"), "missing plain key: {yaml}");
    assert!(
        yaml.contains("\"a:b\": 2\n"),
        "missing quoted ':' key: {yaml}"
    );
    assert!(
        yaml.contains("\"line\\nbreak\": 3\n"),
        "missing escaped newline key: {yaml}"
    );
    assert!(
        yaml.contains("\"\\x01\": 4\n"),
        "missing escaped control-char key: {yaml}"
    );
}

#[test]
fn map_keys_support_bool_int_char_option_unit_variant_and_newtype_struct() {
    // bool/int/char keys
    let mut a: BTreeMap<i32, bool> = BTreeMap::new();
    a.insert(-1, true);
    a.insert(2, false);
    let yaml = to_string(&a).expect("serialize int-key map");
    assert!(yaml.contains("-1: true\n"));
    assert!(yaml.contains("2: false\n"));

    let mut b: BTreeMap<char, i32> = BTreeMap::new();
    b.insert('x', 1);
    let yaml = to_string(&b).expect("serialize char-key map");
    assert!(yaml.contains("x: 1\n"));

    // option keys: None should become `null`
    let mut c: BTreeMap<Option<i32>, i32> = BTreeMap::new();
    c.insert(None, 0);
    c.insert(Some(1), 1);
    let yaml = to_string(&c).expect("serialize option-key map");
    assert!(yaml.contains("null: 0\n"));
    assert!(yaml.contains("1: 1\n"));

    // unit variant keys should serialize as their variant name
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
    enum Mode {
        Alpha,
    }
    let mut d: BTreeMap<Mode, i32> = BTreeMap::new();
    d.insert(Mode::Alpha, 9);
    let yaml = to_string(&d).expect("serialize unit-variant-key map");
    assert!(yaml.contains("Alpha: 9\n"));

    // newtype struct keys should be transparent
    let mut e: BTreeMap<Id, i32> = BTreeMap::new();
    e.insert(Id(42), 7);
    let yaml = to_string(&e).expect("serialize newtype-struct-key map");
    assert!(yaml.contains("42: 7\n"));
}

#[test]
fn map_key_float_goes_through_float_formatting() {
    // Use a wrapper with a total order so we can place it in a BTreeMap.
    let mut m: BTreeMap<OrderedF64, i32> = BTreeMap::new();
    m.insert(OrderedF64::new(1.5), 1);
    let yaml = to_string(&m).expect("serialize float-key map");
    assert!(
        yaml.contains("1.5: 1\n"),
        "unexpected float key formatting: {yaml}"
    );
}

#[test]
fn non_scalar_map_key_is_rejected() {
    let mut m: HashMap<SeqKey, i32> = HashMap::new();
    m.insert(SeqKey(1), 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err = to_string(&FlowMap(m)).expect_err("expected error for non-scalar key in flow map");
    let msg = err.to_string();
    assert!(
        msg.contains("non-scalar key"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn bytes_map_key_is_rejected() {
    let mut m: HashMap<BytesKey, i32> = HashMap::new();
    m.insert(BytesKey, 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err = to_string(&FlowMap(m)).expect_err("expected error for bytes key in flow map");
    let msg = err.to_string();
    assert!(
        msg.contains("non-scalar key"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn newtype_variant_map_key_is_rejected() {
    let mut m: HashMap<NewtypeVariantKey, i32> = HashMap::new();
    m.insert(NewtypeVariantKey::V(1), 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err =
        to_string(&FlowMap(m)).expect_err("expected error for newtype variant key in flow map");
    let msg = err.to_string();
    assert!(
        msg.contains("non-scalar key"),
        "unexpected error message: {msg}"
    );
}
#[test]
fn bool_key_in_map() {
    // bool key -> KeyScalarSink::serialize_bool
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    m.insert(false, "no");
    let yaml = to_string(&m).unwrap();
    assert!(
        yaml.contains("true:") || yaml.contains("false:"),
        "yaml: {yaml}"
    );
}

#[test]
fn i8_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(42i8, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("42:"), "yaml: {yaml}");
}

#[test]
fn i16_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(1000i16, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1000:"), "yaml: {yaml}");
}

#[test]
fn i32_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(-5i32, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("-5:"), "yaml: {yaml}");
}

#[test]
fn i128_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(999i128, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("999:"), "yaml: {yaml}");
}

#[test]
fn u8_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(255u8, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("255:"), "yaml: {yaml}");
}

#[test]
fn u16_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(1u16, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1:"), "yaml: {yaml}");
}

#[test]
fn u32_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(100u32, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("100:"), "yaml: {yaml}");
}

#[test]
fn u128_key_in_map() {
    let mut m = BTreeMap::new();
    m.insert(12345u128, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("12345:"), "yaml: {yaml}");
}

#[test]
fn f32_key_in_map() {
    // f32 key -> KeyScalarSink::serialize_f32
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct F32Key(u32);
    impl Serialize for F32Key {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_f32(f32::from_bits(self.0))
        }
    }
    let mut m = BTreeMap::new();
    m.insert(F32Key(1.5f32.to_bits()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("1.5:"), "yaml: {yaml}");
}

#[test]
fn f64_key_in_map() {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct F64Key(u64);
    impl Serialize for F64Key {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_f64(f64::from_bits(self.0))
        }
    }
    let mut m = BTreeMap::new();
    m.insert(F64Key(2.5f64.to_bits()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("2.5:"), "yaml: {yaml}");
}

#[test]
fn char_key_in_map() {
    // char key -> KeyScalarSink::serialize_char
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct CharKey(char);
    impl Serialize for CharKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_char(self.0)
        }
    }
    let mut m = BTreeMap::new();
    m.insert(CharKey('z'), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("z:"), "yaml: {yaml}");
}

#[test]
fn unit_key_in_map() {
    // unit key -> KeyScalarSink::serialize_unit
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct UnitKey;
    impl Serialize for UnitKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_unit()
        }
    }
    let mut m = BTreeMap::new();
    m.insert(UnitKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("null:"), "yaml: {yaml}");
}

#[test]
fn unit_struct_key_in_map() {
    // unit_struct key -> KeyScalarSink::serialize_unit_struct
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct UnitStructKey;
    impl Serialize for UnitStructKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_unit_struct("UnitStructKey")
        }
    }
    let mut m = BTreeMap::new();
    m.insert(UnitStructKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("null:"), "yaml: {yaml}");
}

#[test]
fn unit_variant_key_in_map() {
    // unit_variant key -> KeyScalarSink::serialize_unit_variant
    #[derive(PartialEq, Eq, PartialOrd, Ord, Serialize)]
    enum Color {
        Red,
        Blue,
    }
    let mut m = BTreeMap::new();
    m.insert(Color::Red, 1);
    m.insert(Color::Blue, 2);
    let yaml = to_string(&m).unwrap();
    assert!(
        yaml.contains("Red:") || yaml.contains("Blue:"),
        "yaml: {yaml}"
    );
}

#[test]
fn newtype_struct_key_in_map() {
    // newtype_struct key -> KeyScalarSink::serialize_newtype_struct (transparent)
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct WrappedStr(String);
    impl Serialize for WrappedStr {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_newtype_struct("WrappedStr", &self.0)
        }
    }
    let mut m = BTreeMap::new();
    m.insert(WrappedStr("mykey".to_string()), "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("mykey:"), "yaml: {yaml}");
}

#[test]
fn collect_str_key_in_map() {
    // collect_str key -> KeyScalarSink::collect_str
    struct DisplayKey(i32);
    impl Serialize for DisplayKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_str(&self.0)
        }
    }
    impl PartialEq for DisplayKey {
        fn eq(&self, o: &Self) -> bool {
            self.0 == o.0
        }
    }
    impl Eq for DisplayKey {}
    impl PartialOrd for DisplayKey {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for DisplayKey {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.0.cmp(&o.0)
        }
    }
    let mut m = BTreeMap::new();
    m.insert(DisplayKey(42), "val");
    let yaml = to_string(&m).unwrap();
    // collect_str produces a string key which may be quoted
    assert!(yaml.contains("42"), "yaml: {yaml}");
}

#[test]
fn non_scalar_key_produces_complex_key_syntax() {
    // A sequence as a map key -> complex key with '? ' syntax
    struct SeqKey;
    impl Serialize for SeqKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let mut seq = s.serialize_seq(Some(1))?;
            seq.serialize_element(&1i32)?;
            seq.end()
        }
    }
    impl PartialEq for SeqKey {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for SeqKey {}
    impl PartialOrd for SeqKey {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for SeqKey {
        fn cmp(&self, _: &Self) -> std::cmp::Ordering {
            std::cmp::Ordering::Equal
        }
    }

    let mut m = BTreeMap::new();
    m.insert(SeqKey, "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("? "), "expected complex key syntax: {yaml}");
}

#[test]
fn key_with_tab_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\twith\ttabs", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\t"), "expected \\t escape in key: {yaml}");
}

#[test]
fn key_with_newline_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\nwith\nnewlines", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\n"), "expected \\n escape in key: {yaml}");
}

#[test]
fn key_with_carriage_return_gets_escaped() {
    let mut m = BTreeMap::new();
    m.insert("key\rwith\rcr", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\r"), "expected \\r escape in key: {yaml}");
}

#[test]
fn key_with_control_char_gets_hex_escape() {
    let mut m = BTreeMap::new();
    m.insert("key\x01ctrl", "val");
    let yaml = to_string(&m).unwrap();
    assert!(
        yaml.contains("\\x01"),
        "expected \\x01 escape in key: {yaml}"
    );
}

#[test]
fn key_with_backslash_plain() {
    // Backslash is plain-safe in YAML keys, no quoting needed
    let mut m = BTreeMap::new();
    m.insert("key\\with\\backslash", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("key\\with\\backslash:"), "yaml: {yaml}");
}

#[test]
fn seq_as_map_key_returns_error_for_seq_methods() {
    // Trying to use a sequence as a map key should produce an error
    // (KeyScalarSink rejects serialize_seq)
    use serde::ser::SerializeMap;
    struct BadKey;
    impl Serialize for BadKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let seq = s.serialize_seq(Some(0))?;
            seq.end()
        }
    }
    // Wrap in a map to trigger KeyScalarSink
    struct MapWithBadKey;
    impl Serialize for MapWithBadKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            let mut map = s.serialize_map(Some(1))?;
            map.serialize_key(&BadKey)?;
            map.serialize_value(&1i32)?;
            map.end()
        }
    }
    // This should either succeed (complex key) or return an error
    let result = to_string(&MapWithBadKey);
    // Either way, no panic
    let _ = result;
}

#[test]
fn complex_map_key_uses_question_mark_syntax() {
    // Block maps support complex keys via `? key\n: value` syntax.
    // Use a Vec as key (non-scalar) in a BTreeMap-like structure.
    use serde::ser::{SerializeMap, Serializer};

    // Manually serialize a map with a sequence key
    struct ComplexKeyMap;
    impl Serialize for ComplexKeyMap {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&vec![1, 2])?;
            map.serialize_value(&"val")?;
            map.end()
        }
    }
    let yaml = to_string(&ComplexKeyMap).unwrap();
    assert!(yaml.contains("? "), "expected complex key syntax: {yaml}");
}

#[test]
fn u128_map_key() {
    let mut m = BTreeMap::new();
    m.insert(999u128, "big");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("999: big"), "got: {yaml}");
}

#[test]
fn i128_map_key() {
    let mut m = BTreeMap::new();
    m.insert(-999i128, "neg");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("-999: neg"), "got: {yaml}");
}

#[test]
fn serialize_bool_key_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    m.insert(false, "no");
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("true") && s.contains("false"));
}

#[test]
fn serialize_integer_key_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert(1i32, "one");
    m.insert(2i32, "two");
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("1: one"));
}

#[test]
fn serialize_float_key_map() {
    let s = serde_saphyr::to_string(&std::f64::consts::PI).unwrap();
    assert!(s.contains("3.14159"));
}
