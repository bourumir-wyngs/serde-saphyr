#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde_saphyr::{DuplicateKeyPolicy, Error};
use std::collections::BTreeMap;

type CompositeMap = BTreeMap<BTreeMap<String, u32>, u32>;
type NullCompositeMap = BTreeMap<BTreeMap<Option<String>, u32>, u32>;

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

fn null_composite_entry(value: u32) -> NullCompositeMap {
    BTreeMap::from([(BTreeMap::from([(None, 1)]), value)])
}

#[test]
fn composite_key_with_actual_null_roundtrips() {
    let expected = null_composite_entry(2);
    let yaml = serde_saphyr::to_string(&expected).unwrap();
    let actual: NullCompositeMap = serde_saphyr::from_str(&yaml).unwrap();

    assert_eq!(actual, expected, "serialized YAML: {yaml}");
}

#[rstest]
#[case::null("null")]
#[case::tilde("~")]
#[case::explicit_null("!!null null")]
#[case::explicit_empty_null("!!null ")]
#[case::empty("")]
fn composite_keys_preserve_actual_null_entries(#[case] spelling: &str) {
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let scalar_yaml = format!("{spelling}: 1\n");
        let scalar: BTreeMap<Option<String>, u32> =
            serde_saphyr::from_str_with_options(&scalar_yaml, options.clone()).unwrap();
        assert_eq!(scalar, BTreeMap::from([(None, 1)]), "{scalar_yaml}");

        let composite_yaml = format!("?\n  {spelling}: 1\n: 2\n");
        let composite: NullCompositeMap =
            serde_saphyr::from_str_with_options(&composite_yaml, options).unwrap();
        assert_eq!(
            composite,
            null_composite_entry(2),
            "{policy:?}: {composite_yaml}"
        );
    }
}

#[test]
fn duplicate_composite_keys_with_actual_null_follow_policy() {
    let yaml = "? {null: 1}\n: 2\n? {null: 1}\n: 3\n";
    let error = serde_saphyr::from_str::<NullCompositeMap>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));

    for (policy, expected_value) in [
        (DuplicateKeyPolicy::FirstWins, 2),
        (DuplicateKeyPolicy::LastWins, 3),
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: NullCompositeMap = serde_saphyr::from_str_with_options(yaml, options).unwrap();
        assert_eq!(actual, null_composite_entry(expected_value), "{policy:?}");
    }
}

#[test]
fn last_wins_buffering_and_merges_preserve_actual_null_composite_keys() {
    for yaml in [
        "1:\n  ? {null: 1}\n  : 2\n",
        "1:\n  <<:\n    ? {null: 1}\n    : 2\n",
    ] {
        let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
        let actual: BTreeMap<u32, NullCompositeMap> =
            serde_saphyr::from_str_with_options(yaml, options).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([(1, null_composite_entry(2))]),
            "{yaml}"
        );
    }
}
