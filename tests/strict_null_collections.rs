#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_saphyr::DuplicateKeyPolicy;
use std::{collections::BTreeMap, fmt::Debug};

#[derive(Debug, Deserialize, PartialEq)]
struct Field<T> {
    items: T,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Defaulted {
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct EmptyTuple();

fn assert_rejected_by_single_document_apis<T: DeserializeOwned + Debug>(yaml: &str) {
    for (api, result) in [
        ("from_str", serde_saphyr::from_str::<T>(yaml)),
        ("from_slice", serde_saphyr::from_slice::<T>(yaml.as_bytes())),
        (
            "from_reader",
            serde_saphyr::from_reader::<_, T>(yaml.as_bytes()),
        ),
    ] {
        assert!(
            result.is_err(),
            "{api} accepted {yaml:?} as {}: {result:?}",
            std::any::type_name::<T>()
        );
    }
}

#[rstest]
#[case::null("null")]
#[case::title_case("Null")]
#[case::uppercase("NULL")]
#[case::tilde("~")]
#[case::empty_stream("")]
#[case::comment_only("# comment\n")]
#[case::empty("---\n")]
#[case::tagged_empty("!!null")]
#[case::tagged_null("!!null null")]
#[case::tagged_tilde("!!null ~")]
fn null_is_not_an_empty_collection(#[case] yaml: &str) {
    assert_rejected_by_single_document_apis::<Vec<String>>(yaml);
    assert_rejected_by_single_document_apis::<[String; 0]>(yaml);
    assert_rejected_by_single_document_apis::<EmptyTuple>(yaml);
    assert_rejected_by_single_document_apis::<BTreeMap<String, String>>(yaml);
    assert_rejected_by_single_document_apis::<Defaulted>(yaml);

    // Null remains valid for optional and unit targets.
    assert_eq!(
        serde_saphyr::from_str::<Option<Vec<String>>>(yaml).unwrap(),
        None
    );
    assert_eq!(
        serde_saphyr::from_str::<Option<BTreeMap<String, String>>>(yaml).unwrap(),
        None
    );
    serde_saphyr::from_str::<()>(yaml).unwrap();
}

#[rstest]
#[case::null("items: null\n")]
#[case::omitted_value("items:\n")]
#[case::tilde("items: ~\n")]
fn default_only_applies_to_missing_fields(#[case] yaml: &str) {
    assert_rejected_by_single_document_apis::<Defaulted>(yaml);
    assert_rejected_by_single_document_apis::<Field<Vec<String>>>(yaml);
    assert_eq!(
        serde_saphyr::from_str::<Field<Option<Vec<String>>>>(yaml)
            .unwrap()
            .items,
        None
    );
}

#[test]
fn missing_fields_keep_serde_default_and_option_semantics() {
    assert!(serde_saphyr::from_str::<Field<Vec<String>>>("{}").is_err());
    assert!(
        serde_saphyr::from_str::<Defaulted>("{}")
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        serde_saphyr::from_str::<Field<Option<Vec<String>>>>("{}")
            .unwrap()
            .items,
        None
    );
}

#[test]
fn explicit_empty_collections_remain_valid() {
    assert!(
        serde_saphyr::from_str::<Vec<String>>("[]")
            .unwrap()
            .is_empty()
    );
    assert!(
        serde_saphyr::from_str::<[String; 0]>("[]")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        serde_saphyr::from_str::<EmptyTuple>("[]").unwrap(),
        EmptyTuple()
    );
    assert!(
        serde_saphyr::from_str::<BTreeMap<String, String>>("{}")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        serde_saphyr::from_str::<Defaulted>("items: []\n").unwrap(),
        Defaulted { items: Vec::new() }
    );
    assert_eq!(
        serde_saphyr::from_str::<Field<Option<Vec<String>>>>("items: []\n")
            .unwrap()
            .items,
        Some(Vec::new())
    );
}

#[rstest]
#[case::alias("anchor: &empty null\nitems: *empty\n", DuplicateKeyPolicy::Error)]
#[case::empty_alias("anchor: &empty\nitems: *empty\n", DuplicateKeyPolicy::Error)]
#[case::last_wins("items: [first]\nitems: null\n", DuplicateKeyPolicy::LastWins)]
#[case::last_wins_empty("items: [first]\nitems:\n", DuplicateKeyPolicy::LastWins)]
fn replayed_null_is_not_an_empty_collection(
    #[case] yaml: &str,
    #[case] policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! { duplicate_keys: policy };
    assert!(
        serde_saphyr::from_str_with_options::<Field<Vec<String>>>(yaml, options.clone()).is_err()
    );
    assert!(
        serde_saphyr::from_str_with_options::<Field<BTreeMap<String, String>>>(
            yaml,
            options.clone()
        )
        .is_err()
    );
    assert!(
        serde_saphyr::from_str_with_options::<Field<Defaulted>>(yaml, options.clone()).is_err()
    );
    assert_eq!(
        serde_saphyr::from_str_with_options::<Field<Option<Vec<String>>>>(yaml, options)
            .unwrap()
            .items,
        None
    );
}
