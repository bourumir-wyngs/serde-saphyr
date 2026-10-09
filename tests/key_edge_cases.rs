#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde_saphyr::{DuplicateKeyPolicy, Error};
use std::collections::HashMap;

/// Unsure if this should be error. When forcing into string, empty key is currently
/// deserialized into unit ('~')
#[test]
fn deserialize_empty_key_into_hashmap_string() {
    // Single mapping entry with an empty key
    let y = ": value\n";
    let m: HashMap<Option<String>, Option<String>> =
        serde_saphyr::from_str(y).expect("deserialization error");
    assert_eq!(m.get(&None), Some(&Some("value".to_string())));
}

#[test]
fn deserialize_empty_key_into_hashmap_option() {
    // Single mapping entry with an empty key
    let y = ": value\n";
    let m: HashMap<Option<String>, String> =
        serde_saphyr::from_str(y).expect("failed to parse empty-key mapping");

    assert_eq!(m.len(), 1);
    assert_eq!(m.get(&None), Some(&"value".to_string()));
}

#[test]
fn duplicate_empty_keys_follow_configured_policy() {
    let yaml = ": a\n: b\n";

    let error = serde_saphyr::from_str::<HashMap<Option<String>, String>>(yaml)
        .expect_err("the default duplicate-key policy must reject the second empty key");
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey {
            key: Some(key),
            ..
        } if key == "~"
    ));

    let first_wins = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };
    let first =
        serde_saphyr::from_str_with_options::<HashMap<Option<String>, String>>(yaml, first_wins)
            .expect("FirstWins must accept duplicate empty keys");
    assert_eq!(first, HashMap::from([(None, "a".to_owned())]));

    let last_wins = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::LastWins,
    };
    let last =
        serde_saphyr::from_str_with_options::<HashMap<Option<String>, String>>(yaml, last_wins)
            .expect("LastWins must accept duplicate empty keys");
    assert_eq!(last, HashMap::from([(None, "b".to_owned())]));
}

#[test]
fn deserialize_empty_key_into_json_null() {
    // Single mapping entry with an empty key
    let y = ": value\n";
    let m: Result<serde_json::Value, Error> = serde_saphyr::from_str(y);
    assert!(
        m.is_err(),
        "Empty key is not valid JSON key because it is not valid string value"
    );
}

#[test]
fn deserialize_quoted_key_into_hashmap_string() {
    // Single mapping entry with an empty key
    let y = "\"\": value\n";
    let m: HashMap<String, String> =
        serde_saphyr::from_str(y).expect("failed to parse empty-key mapping");

    assert_eq!(m.len(), 1);
    assert_eq!(m.get(""), Some(&"value".to_string()));
}

#[test]
fn deserialize_null_key_into_hashmap_option_string() {
    // Null scalar key (~) should map to None when targeting Option<String>
    let y = "~: value\n";
    let m: HashMap<Option<String>, String> =
        serde_saphyr::from_str(y).expect("failed to parse null-key mapping");

    assert_eq!(m.len(), 1);
    assert_eq!(m.get(&None), Some(&"value".to_string()));
}

#[test]
fn deserialize_unit_key_into_hashmap_unit() {
    // In Serde, the unit type `()` is represented as YAML null. Using `~` as the key
    // should deserialize into the unit value when targeting `HashMap<(), String>`.
    let y = "~: value\n";
    let m: HashMap<(), String> =
        serde_saphyr::from_str(y).expect("failed to parse unit-key mapping");

    assert_eq!(m.len(), 1);
    assert_eq!(m.get(&()), Some(&"value".to_string()));
}

/// Block collections used as map keys are written after `? `; their continuation lines must
/// align with the first item (two columns past `?`), at the top level and nested.
#[test]
fn block_collection_keys_round_trip() {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum E {
        C { x: String, y: i32 },
    }

    fn round_trip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        for indent_step in 1..=9 {
            for compact_list_indent in [false, true] {
                let options = serde_saphyr::ser_options! {
                    indent_step: indent_step,
                    compact_list_indent: compact_list_indent,
                };
                let context =
                    format!("indent_step {indent_step}, compact_list_indent {compact_list_indent}");
                let yaml = serde_saphyr::to_string_with_options(value, options)
                    .unwrap_or_else(|e| panic!("{context}: {e}"));
                let back: T = serde_saphyr::from_str(&yaml)
                    .unwrap_or_else(|e| panic!("{context}: {e}\n{yaml}"));
                assert_eq!(&back, value, "{context}:\n{yaml}");
            }
        }
    }

    round_trip(&BTreeMap::from([((1, "a".to_string()), 0)]));
    round_trip(&BTreeMap::from([(vec![1, 2], 0)]));
    round_trip(&BTreeMap::from([(
        E::C {
            x: "y".into(),
            y: 1,
        },
        0,
    )]));
    round_trip(&BTreeMap::from([((1, 2), (3, 4))]));
    round_trip(&BTreeMap::from([(
        "outer".to_string(),
        BTreeMap::from([((1, "a".to_string()), 0)]),
    )]));
    round_trip(&vec![BTreeMap::from([((1, "a".to_string()), 0)])]);

    let yaml = serde_saphyr::to_string(&BTreeMap::from([((1, "a".to_string()), 0)])).unwrap();
    assert_eq!(yaml, "? - 1\n  - a\n: 0\n");
}
