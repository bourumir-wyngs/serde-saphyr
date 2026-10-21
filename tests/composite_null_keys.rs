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

#[rstest]
#[case::implicit("null")]
#[case::explicit("!!null null")]
#[case::quoted_explicit("!!null 'null'")]
fn duplicate_composite_keys_with_actual_null_follow_policy(#[case] second_key: &str) {
    let yaml = format!("? {{null: 1}}\n: 2\n? {{{second_key}: 1}}\n: 3\n");
    let error = serde_saphyr::from_str::<NullCompositeMap>(&yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));

    for (policy, expected_value) in [
        (DuplicateKeyPolicy::FirstWins, 2),
        (DuplicateKeyPolicy::LastWins, 3),
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: NullCompositeMap = serde_saphyr::from_str_with_options(&yaml, options).unwrap();
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

#[rstest]
#[case::direct("")]
#[case::merged("<<:\n")]
fn composite_keys_preserve_distinct_null_like_strings(#[case] prefix: &str) {
    let yaml = format!(
        "{prefix}  ? {{!!str null: 1}}\n  : 20\n  ? {{'~': 1}}\n  : 30\n  ? {{'': 1}}\n  : 40\n"
    );
    let expected = BTreeMap::from([
        (BTreeMap::from([("null".to_owned(), 1)]), 20),
        (BTreeMap::from([("~".to_owned(), 1)]), 30),
        (BTreeMap::from([(String::new(), 1)]), 40),
    ]);

    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: CompositeMap = serde_saphyr::from_str_with_options(&yaml, options).unwrap();
        assert_eq!(actual, expected, "{policy:?}: {yaml}");
    }

    let serialized = serde_saphyr::to_string(&expected).unwrap();
    let round_trip: CompositeMap = serde_saphyr::from_str(&serialized).unwrap();
    assert_eq!(round_trip, expected, "serialized YAML: {serialized}");
}

#[rstest]
#[case::explicit_string("null", "!!str null", "null")]
#[case::quoted_string("null", "'null'", "null")]
#[case::non_specific_string("null", "! null", "null")]
#[case::uppercase("NULL", "'NULL'", "NULL")]
#[case::tilde("~", "'~'", "~")]
#[case::empty("", "''", "")]
#[case::explicit_null("!!null null", "!!str null", "null")]
#[case::quoted_explicit_null("!!null 'null'", "'null'", "null")]
fn composite_keys_distinguish_actual_null_from_string_null(
    #[case] null_key: &str,
    #[case] string_key: &str,
    #[case] text: &str,
) {
    let entries = format!("  ? {{{null_key}: 1}}\n  : 10\n  ? {{{string_key}: 1}}\n  : 20\n");
    let expected = BTreeMap::from([
        (BTreeMap::from([(None, 1)]), 10),
        (BTreeMap::from([(Some(text.to_owned()), 1)]), 20),
    ]);
    for prefix in ["", "<<:\n"] {
        let yaml = format!("{prefix}{entries}");
        for policy in [
            DuplicateKeyPolicy::Error,
            DuplicateKeyPolicy::FirstWins,
            DuplicateKeyPolicy::LastWins,
        ] {
            let options = serde_saphyr::options! { duplicate_keys: policy };
            let actual: NullCompositeMap =
                serde_saphyr::from_str_with_options(&yaml, options.clone()).unwrap();
            assert_eq!(actual, expected, "{policy:?}: {yaml}");

            let actual: NullCompositeMap =
                serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap();
            assert_eq!(actual, expected, "reader, {policy:?}: {yaml}");
        }
    }
}

#[rstest]
#[case::scalar("null: 10\n!!str null: 20\n")]
#[case::empty_scalar(": 10\n'': 20\n")]
#[case::literal("null: 10\n? |-\n  null\n: 20\n")]
#[case::folded("null: 10\n? >-\n  null\n: 20\n")]
#[case::empty_literal(": 10\n? |\n: 20\n")]
#[case::empty_folded(": 10\n? >\n: 20\n")]
fn scalar_keys_distinguish_actual_null_from_string_null(#[case] yaml: &str) {
    let text = if yaml.starts_with(':') { "" } else { "null" };
    let expected = BTreeMap::from([(None, 10), (Some(text.to_owned()), 20)]);
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: BTreeMap<Option<String>, u32> =
            serde_saphyr::from_str_with_options(yaml, options).unwrap();
        assert_eq!(actual, expected, "{policy:?}: {yaml}");
    }
}

#[rstest]
#[case::sequence("? [null]\n: 10\n? [!!str null]\n: 20\n")]
#[case::mapping_value("? {key: null}\n: 10\n? {key: !!str null}\n: 20\n")]
fn null_and_string_identity_applies_throughout_composite_keys(#[case] yaml: &str) {
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        if yaml.starts_with("? [") {
            let actual: BTreeMap<Vec<Option<String>>, u32> =
                serde_saphyr::from_str_with_options(yaml, options).unwrap();
            assert_eq!(
                actual,
                BTreeMap::from([(vec![None], 10), (vec![Some("null".to_owned())], 20)]),
                "{policy:?}: {yaml}"
            );
        } else {
            let actual: BTreeMap<BTreeMap<String, Option<String>>, u32> =
                serde_saphyr::from_str_with_options(yaml, options).unwrap();
            assert_eq!(
                actual,
                BTreeMap::from([
                    (BTreeMap::from([("key".to_owned(), None)]), 10),
                    (
                        BTreeMap::from([("key".to_owned(), Some("null".to_owned()))]),
                        20
                    ),
                ]),
                "{policy:?}: {yaml}"
            );
        }
    }
}

#[test]
fn custom_tagged_null_keys_keep_their_tag_identity() {
    let yaml = "? {null: 1}\n: 10\n? {!First null: 1}\n: 20\n? {!Second null: 1}\n: 30\n";
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: NullCompositeMap = serde_saphyr::from_str_with_options(yaml, options).unwrap();
        // These are distinct YAML keys. The target's Option<String> conversion
        // discards their tags, so the BTreeMap itself retains the final value.
        assert_eq!(actual, null_composite_entry(30), "{policy:?}");
    }
}

#[test]
fn aliased_sequence_keys_preserve_null_strings_and_nested_values() {
    type SequenceMap = BTreeMap<Vec<Option<String>>, Vec<u32>>;

    let yaml =
        "? &key [!!str null, '~', '', null]\n: [1, 2]\n? *key\n: [3, 4, 5]\n? [tail]\n: [6]\n";
    let error = serde_saphyr::from_str::<SequenceMap>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));

    for (policy, selected_value) in [
        (DuplicateKeyPolicy::FirstWins, vec![1, 2]),
        (DuplicateKeyPolicy::LastWins, vec![3, 4, 5]),
    ] {
        let expected = BTreeMap::from([
            (
                vec![
                    Some("null".to_owned()),
                    Some("~".to_owned()),
                    Some(String::new()),
                    None,
                ],
                selected_value,
            ),
            (vec![Some("tail".to_owned())], vec![6]),
        ]);
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: SequenceMap = serde_saphyr::from_str_with_options(yaml, options).unwrap();
        assert_eq!(actual, expected, "{policy:?}");
    }
}
