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
