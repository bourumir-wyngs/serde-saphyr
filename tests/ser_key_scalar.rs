use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use serde::ser::{SerializeSeq, Serializer};
use serde::Serialize;

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
    assert!(yaml.contains("\"a:b\": 2\n"), "missing quoted ':' key: {yaml}");
    assert!(
        yaml.contains("\"line\\nbreak\": 3\n"),
        "missing escaped newline key: {yaml}"
    );
    assert!(
        yaml.contains("\"\\u0001\": 4\n"),
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
    assert!(yaml.contains("1.5: 1\n"), "unexpected float key formatting: {yaml}");
}

#[test]
fn non_scalar_map_key_is_rejected() {
    let mut m: HashMap<SeqKey, i32> = HashMap::new();
    m.insert(SeqKey(1), 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err = to_string(&FlowMap(m)).expect_err("expected error for non-scalar key in flow map");
    let msg = err.to_string();
    assert!(msg.contains("non-scalar key"), "unexpected error message: {msg}");
}

#[test]
fn bytes_map_key_is_rejected() {
    let mut m: HashMap<BytesKey, i32> = HashMap::new();
    m.insert(BytesKey, 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err = to_string(&FlowMap(m)).expect_err("expected error for bytes key in flow map");
    let msg = err.to_string();
    assert!(msg.contains("non-scalar key"), "unexpected error message: {msg}");
}

#[test]
fn newtype_variant_map_key_is_rejected() {
    let mut m: HashMap<NewtypeVariantKey, i32> = HashMap::new();
    m.insert(NewtypeVariantKey::V(1), 1);

    // Block mappings support complex keys (`? ...`), so force flow style where keys must be scalars.
    let err =
        to_string(&FlowMap(m)).expect_err("expected error for newtype variant key in flow map");
    let msg = err.to_string();
    assert!(msg.contains("non-scalar key"), "unexpected error message: {msg}");
}
