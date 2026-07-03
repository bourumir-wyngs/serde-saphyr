#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_saphyr::{from_str, to_string};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum TupleVariant {
    Pair(BTreeMap<String, i64>, Option<i64>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum BoolishVariant {
    #[serde(rename = "yes")]
    Newtype(i32),
    #[serde(rename = "no")]
    Tuple(i32, i32),
    #[serde(rename = "on")]
    Struct { value: i32 },
}

/// Round-trips every printable ASCII single-character key (32..=126)
/// through serde_saphyr using the same pattern as the provided snippet.
/// This ensures that characters like comma `,` are serialized in a way
/// that can be parsed back into the same HashMap.
#[test]
fn printable_ascii_single_char_keys_roundtrip() {
    for c in 32_u8..=126 {
        let s = String::from_utf8(vec![c]).expect("valid UTF-8 single byte");
        let mut h = HashMap::new();
        h.insert(s.clone(), s.clone());

        // NOTE: using the same pattern as the snippet:
        // serialize then deserialize back into a HashMap<String, String>.
        // If a key (e.g., ",") requires quoting, the serializer must emit it.
        let yaml =
            to_string(&serde_saphyr::FlowSeq(h.clone())).expect("serialize FlowSeq<HashMap<..>>");
        let parsed: HashMap<String, String> =
            from_str(&yaml).unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

        assert_eq!(parsed, h, "Round-trip failed for key {:?}", s);
    }
}

/// Focused check for a comma key. This ensures that a map with `,` as a key
/// survives serialization and deserialization exactly.
#[test]
fn specific_key_roundtrip() {
    let mut h = HashMap::new();
    h.insert(",".to_string(), ",".to_string());
    h.insert("".to_string(), " ".to_string()); // empty key
    h.insert("null".to_string(), " ".to_string()); // null key

    // Serialize with the same FlowSeq wrapper as in the original snippet.
    let yaml =
        to_string(&serde_saphyr::FlowSeq(h.clone())).expect("serialize FlowSeq<HashMap<..>>");

    // It must deserialize back to the identical map.
    let parsed: HashMap<String, String> =
        from_str(&yaml).unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

    assert_eq!(parsed, h, "Comma key/value did not round-trip as expected");
}

#[rstest]
#[case(1023)]
#[case(1024)]
#[case(1025)]
#[case(2000)]
fn over_long_keys_roundtrip(#[case] len: usize) {
    // A control char forces quoting, so the key is at least `len` chars long.
    let key = "\u{7f}".repeat(len);
    let mut h = HashMap::new();
    h.insert(key.clone(), "v".to_string());
    let mut outer = HashMap::new();
    outer.insert("wrap".to_string(), h.clone());

    let yaml = to_string(&outer).expect("serialize map with over-long key");
    let parsed: HashMap<String, HashMap<String, String>> = from_str(&yaml)
        .unwrap_or_else(|e| panic!("deserialize over-long key (len {len}) failed: {e}\n{yaml}"));
    assert_eq!(
        parsed, outer,
        "over-long key (len {len}) did not round-trip"
    );
}

/// A key past the 1024-char simple-key limit is emitted with the explicit `? key`
/// form; its tuple-variant value must stay indented under the variant rather than
/// dropping the first sequence dash to column 0.
#[test]
fn over_long_key_with_tuple_variant_value() {
    let inner = BTreeMap::from([("a".to_string(), 1), ("b".to_string(), 2)]);
    let value = TupleVariant::Pair(inner, None);

    let mut m = BTreeMap::new();
    m.insert("p".repeat(1100), value);

    let yaml = to_string(&m).expect("serialize map with over-long explicit key");
    let back: BTreeMap<String, TupleVariant> = from_str(&yaml)
        .unwrap_or_else(|e| panic!("explicit-key tuple value failed to parse back: {e}\n{yaml}"));
    assert_eq!(back, m);
}

#[test]
fn enum_variant_mapping_keys_quote_yaml11_boolean_spellings() {
    let cases = [
        (
            BoolishVariant::Newtype(1),
            "\"yes\": 1\n",
            "newtype variant",
        ),
        (
            BoolishVariant::Tuple(1, 2),
            "\"no\":\n  - 1\n  - 2\n",
            "tuple variant",
        ),
        (
            BoolishVariant::Struct { value: 1 },
            "\"on\":\n  value: 1\n",
            "struct variant",
        ),
    ];

    for (value, expected, label) in cases {
        let yaml = to_string(&value).unwrap_or_else(|err| panic!("serialize {label}: {err}"));
        assert_eq!(yaml, expected, "{label} output changed");

        let back: BoolishVariant =
            from_str(&yaml).unwrap_or_else(|err| panic!("deserialize {label}: {err}\n{yaml}"));
        assert_eq!(back, value, "{label} did not round-trip");
    }
}

#[test]
fn yaml12_enum_variant_mapping_keys_leave_yaml11_only_bools_plain() {
    let yaml = serde_saphyr::to_string_with_options(
        &BoolishVariant::Newtype(1),
        serde_saphyr::ser_options! { yaml_12: true },
    )
    .expect("serialize yaml 1.2 newtype variant");

    assert!(
        yaml.starts_with("%YAML 1.2\n---\n"),
        "missing YAML 1.2 directive: {yaml}"
    );
    assert!(
        yaml.contains("\nyes: 1\n"),
        "YAML 1.1-only bool spelling should stay plain in YAML 1.2 mode: {yaml}"
    );
    assert!(
        !yaml.contains("\"yes\""),
        "YAML 1.2 mode should not quote yes variant key: {yaml}"
    );

    let back: BoolishVariant =
        from_str(&yaml).unwrap_or_else(|err| panic!("deserialize yaml 1.2 variant: {err}\n{yaml}"));
    assert_eq!(back, BoolishVariant::Newtype(1));
}

#[test]
fn whitespace_padded_keys_roundtrip() {
    let mut h = HashMap::new();
    // if we were to trim or not quote the keys, they would collapse
    h.insert("foo ".to_string(), "trailing".to_string());
    h.insert("foo".to_string(), "bare".to_string());
    h.insert(" foo".to_string(), "leading".to_string());
    h.insert("foo bar".to_string(), "inner".to_string());

    let yaml = to_string(&h).expect("serialize HashMap with whitespace-padded keys");

    let parsed: HashMap<String, String> = from_str(&yaml).unwrap();
    assert_eq!(parsed, h);
}

/// Ensures that string keys that look like numbers ("1", "2.42") are quoted
/// during serialization so they round-trip as strings, not numbers.
#[test]
fn numeric_string_keys_roundtrip() {
    let mut map = HashMap::new();
    map.insert("1".to_string(), "value1".to_string());
    map.insert("2".to_string(), "value2".to_string());
    map.insert("42".to_string(), "value42".to_string());
    map.insert("-5".to_string(), "negative".to_string());
    map.insert("3.14".to_string(), "pi".to_string());
    // Oversized numeric-looking keys that can exceed common integer/float parsing ranges.
    let huge_int = "9".repeat(200);
    let huge_float_exp = "1e99999999".to_string();
    map.insert(huge_int.clone(), "huge_int".to_string());
    map.insert(huge_float_exp.clone(), "huge_float_exp".to_string());

    let yaml = to_string(&map).expect("serialize HashMap with numeric string keys");

    // The keys should be quoted in the YAML output
    for value in [
        "1",
        "2",
        "42",
        "-5",
        "3.14",
        huge_int.as_str(),
        huge_float_exp.as_str(),
    ] {
        assert!(
            yaml.contains(&format!("\"{value}\"")),
            "Key '{}' should be quoted in YAML output, got:\n{}",
            value,
            yaml
        );
    }
    // Verify they round-trip correctly as strings
    let parsed: HashMap<String, String> =
        from_str(&yaml).unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

    assert_eq!(parsed, map);
}
