use serde::Deserialize;
use std::hash::{Hash, Hasher};

/// Canon: a tiny canonical value type used only in tests to assert YAML structure.
///
/// Why we use this instead of concrete Rust types:
/// - Some YAML edge cases (especially around explicit keys introduced with `?` and
///   empty/null scalars) do not map cleanly to a single Rust type without making
///   assumptions about the parser’s internal event shapes.
/// - Production types like `HashMap<String, _>` or even `serde_json::Value` either
///   over-constrain the shape (e.g., requiring scalar string keys) or bring in
///   semantics we do not want to couple to here.
/// - `serde::de::IgnoredAny` is useful to assert “it parses”, but it discards the
///   structure entirely. Canon sits in between: it captures enough of the YAML
///   shape to write meaningful assertions while remaining independent of the
///   library’s internal representations.
///
/// What Canon represents:
/// - Null: YAML null forms (tagged null, `~`, `null`, empty plain scalar) are
///   represented as `Canon::Null` via Serde’s unit/option callbacks.
/// - String: plain or quoted scalars that we treat as strings end up as
///   `Canon::String`.
/// - Seq: YAML sequences become `Canon::Seq` of nested Canon values.
/// - Map: YAML mappings become `Canon::Map` with a deterministic Vec of `(key, value)`
///   pairs in the order they are encountered. Canon implements `Eq` and `Hash` so it
///   can be used as a key in `HashMap<Canon, Canon>` inside tests when we want to
///   assert complex keys.
///
/// How we use it in these tests:
/// - M2N8 Case 1 ("- ? : x"): The explicit key indicator `?` with nothing before `:`
///   denotes an empty key node. Our production deserializer, when deserializing into
///   `HashMap<Option<String>, String>`, maps that key to `None` and the value to "x".
///   Canon is not needed there, but provides a fallback when we want to assert shapes
///   without enforcing a particular Rust key type.
/// - M2N8 Case 2 ("? []: x\n:\n"): The first key is itself a mapping `{ []: x }` and
///   there is also an empty key. Using `HashMap<Canon, Canon>` lets us assert the
///   presence of that complex key and its null value, and optionally a second `null`
///   key, without being sensitive to internal event modeling.
///
/// Scope and limitations:
/// - Canon is intentionally minimal; it is not a full YAML value type and should not
///   be exposed from the crate. It exists only in tests.
/// - Ordering of `Map` entries is the source order captured in a Vec, which is
///   sufficient for the assertions we write here.
/// - We do not attempt to encode tags, anchors, or numeric/boolean distinctions; tests
///   that need those should use more specific Rust types.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Canon {
    Null,
    String(String),
    Seq(Vec<Canon>),
    Map(Vec<(Canon, Canon)>),
}

impl Hash for Canon {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Canon::Null => {
                0u8.hash(state);
            }
            Canon::String(s) => {
                1u8.hash(state);
                s.hash(state);
            }
            Canon::Seq(v) => {
                2u8.hash(state);
                v.hash(state);
            }
            Canon::Map(entries) => {
                3u8.hash(state);
                entries.hash(state);
            }
        }
    }
}

impl<'de> Deserialize<'de> for Canon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Canon;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any YAML value")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Canon::Null)
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Canon::Null)
            }
            fn visit_some<D2>(self, d: D2) -> Result<Self::Value, D2::Error>
            where
                D2: serde::Deserializer<'de>,
            {
                Canon::deserialize(d)
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Canon::String(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Canon::String(v))
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(elem) = seq.next_element::<Canon>()? {
                    out.push(elem);
                }
                Ok(Canon::Seq(out))
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::new();
                while let Some((k, v)) = map.next_entry::<Canon, Canon>()? {
                    entries.push((k, v));
                }
                Ok(Canon::Map(entries))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl std::fmt::Display for Canon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Canon::Null => write!(f, "null"),
            Canon::String(s) => f.write_str(s),
            Canon::Seq(items) => {
                f.write_str("[")?;
                let mut first = true;
                for item in items {
                    if !first {
                        f.write_str(", ")?;
                    } else {
                        first = false;
                    }
                    write!(f, "{}", item)?;
                }
                f.write_str("]")
            }
            Canon::Map(entries) => {
                f.write_str("{")?;
                let mut first = true;
                for (k, v) in entries {
                    if !first {
                        f.write_str(", ")?;
                    } else {
                        first = false;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                f.write_str("}")
            }
        }
    }
}

// M2N8: Question mark edge cases
// Case 1: a sequence with one mapping that uses an explicit empty key ("- ? : x").
//         This denotes a mapping with a single pair: key = empty scalar (null), value = "x".
// Case 2: a mapping where the first key is itself a mapping { []: x } and there is
// an additional empty key with empty value. For this case we currently only assert
// that the document parses successfully; detailed structural assertions can be added
// later as parser/deserializer support for complex/empty explicit keys improves.

#[test]
fn yaml_m2n8_case1_sequence_with_explicit_empty_key_parses() {
    let y1 = "- ? : x\n";

    // Concretely assert the parsed structure: map key should be None (empty/null scalar) and value "x".
    type Item = std::collections::HashMap<Option<String>, String>;
    let docs: Vec<Item> = serde_saphyr::from_str(y1).expect("M2N8 case1 should parse");

    assert_eq!(docs.len(), 1, "expected one sequence element");
    let map = &docs[0];
    assert_eq!(map.len(), 1, "expected one pair in the mapping");
    let (k, v) = map.iter().next().unwrap();
    assert!(k.is_none(), "key should be None (empty/null scalar)");
    assert_eq!(v, "x");
}

#[test]
fn yaml_m2n8_case2_mapping_with_complex_key_shape() {
    // Mapping with two pairs:
    // 1) key = { []: x } (a mapping used as the key), value = null
    // 2) key = null (empty scalar), value = null (empty)
    let y2 = "? []: x\n:\n";

    use std::collections::HashMap;
    let doc: HashMap<Canon, Canon> = serde_saphyr::from_str(y2).expect("M2N8 case2 should parse");

    // Build the expected complex key: a mapping with one pair: [] -> "x"
    let complex_key = Canon::Map(vec![(Canon::Seq(vec![]), Canon::String("x".to_string()))]);

    // Some parsers surface this as a single entry: key = {[]: x}, value = null.
    // Others may also include a second entry with an explicit empty key mapping to null.
    match doc.len() {
        1 => {
            let v1 = doc
                .get(&complex_key)
                .expect("expected complex mapping key {[]: x}");
            // The value side is empty in the YAML after the complex key, which is YAML null.
            // We assert it is deserialized as Canon::Null.
            assert_eq!(v1, &Canon::Null, "value for complex key should be null");
        }
        2 => {
            let v1 = doc
                .get(&complex_key)
                .expect("expected complex mapping key {[]: x}");
            // The value side is empty after the complex key -> YAML null.
            assert_eq!(v1, &Canon::Null, "value for complex key should be null");
            // Some parser event shapes also surface an additional empty key with empty value: null: null.
            let v2 = doc
                .get(&Canon::Null)
                .expect("expected empty (null) key present");
            assert_eq!(v2, &Canon::Null, "value for empty key should be null");
        }
        n => panic!("unexpected number of entries: {}", n),
    }

    // Pretty-print using Canon's Display to show the structure as YAML-like text.
    for (k, v) in &doc {
        println!("{}: {}", k, v);
    }
}
