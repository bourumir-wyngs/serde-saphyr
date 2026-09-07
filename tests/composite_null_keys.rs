#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde_saphyr::{DuplicateKeyPolicy, Error};
use std::collections::BTreeMap;

type CompositeMap = BTreeMap<BTreeMap<String, u32>, u32>;

fn composite_entry(key: &str, value: u32) -> CompositeMap {
    BTreeMap::from([(BTreeMap::from([(key.to_owned(), 1)]), value)])
}

#[test]
fn composite_key_with_null_string_roundtrips() {
    let expected = composite_entry("null", 2);
    let yaml = serde_saphyr::to_string(&expected).unwrap();
    let actual: CompositeMap = serde_saphyr::from_str(&yaml).unwrap();

    assert_eq!(actual, expected, "serialized YAML: {yaml}");
}

#[test]
fn composite_keys_preserve_null_like_strings() {
    for (spelling, key) in [
        ("\"null\"", "null"),
        ("'null'", "null"),
        ("'~'", "~"),
        ("''", ""),
        ("!!str null", "null"),
        ("! null", "null"),
        ("! ~", "~"),
    ] {
        let yaml = format!("? {{{spelling}: 1}}\n: 2\n");
        for policy in [
            DuplicateKeyPolicy::Error,
            DuplicateKeyPolicy::FirstWins,
            DuplicateKeyPolicy::LastWins,
        ] {
            let options = serde_saphyr::options! { duplicate_keys: policy };
            let actual: CompositeMap = serde_saphyr::from_str_with_options(&yaml, options).unwrap();

            assert_eq!(actual, composite_entry(key, 2), "{policy:?}: {yaml}");
        }
    }
}

#[test]
fn duplicate_composite_keys_with_null_strings_follow_policy() {
    let yaml = "? {\"null\": 1}\n: 2\n? {!!str null: 1}\n: 3\n";
    let error = serde_saphyr::from_str::<CompositeMap>(yaml).unwrap_err();
    assert!(
        matches!(error.without_snippet(), Error::DuplicateMappingKey { .. }),
        "{error:?}"
    );

    for (policy, expected_value) in [
        (DuplicateKeyPolicy::FirstWins, 2),
        (DuplicateKeyPolicy::LastWins, 3),
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: CompositeMap = serde_saphyr::from_str_with_options(yaml, options).unwrap();

        assert_eq!(
            actual,
            composite_entry("null", expected_value),
            "{policy:?}"
        );
    }
}

#[test]
fn last_wins_buffering_preserves_null_strings_in_composite_keys() {
    let yaml = "1:\n  ? {\"null\": 1}\n  : 2\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let actual: BTreeMap<u32, CompositeMap> =
        serde_saphyr::from_str_with_options(yaml, options).unwrap();

    assert_eq!(actual, BTreeMap::from([(1, composite_entry("null", 2))]));
}
