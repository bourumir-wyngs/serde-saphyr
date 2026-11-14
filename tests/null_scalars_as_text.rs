#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;
use std::fmt::Debug;

use rstest::rstest;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_saphyr::{
    DuplicateKeyPolicy, Error, Options, from_reader_with_options, from_str_with_options,
};

#[derive(Debug, Deserialize, PartialEq)]
struct Field<T> {
    items: T,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
struct Defaulted {
    #[serde(default)]
    items: Vec<String>,
    label: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Required {
    #[serde(rename = "name")]
    _name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct EmptyTuple();

fn as_text() -> Options {
    serde_saphyr::options! { null_scalars_as_text: true }
}

/// Deserialize from both string and reader input, which must agree.
fn read<T: DeserializeOwned + Debug>(yaml: &str, options: Options) -> Result<T, Error> {
    let from_str = from_str_with_options::<T>(yaml, options.clone());
    let from_reader = from_reader_with_options::<_, T>(yaml.as_bytes(), options);
    assert_eq!(
        format!("{:?}", from_str.as_ref().map_err(|_| ())),
        format!("{:?}", from_reader.as_ref().map_err(|_| ())),
        "string and reader input disagree for {yaml:?}"
    );
    from_str
}

#[rstest]
#[case::empty("items:\n", "")]
#[case::tilde("items: ~\n", "~")]
#[case::lowercase("items: null\n", "null")]
#[case::title_case("items: Null\n", "Null")]
#[case::uppercase("items: NULL\n", "NULL")]
fn null_like_scalar_into_string_is_its_text(#[case] yaml: &str, #[case] text: &str) {
    assert!(matches!(
        read::<Field<String>>(yaml, Options::default())
            .unwrap_err()
            .without_snippet(),
        Error::NullIntoString { .. }
    ));
    assert_eq!(read::<Field<String>>(yaml, as_text()).unwrap().items, text);
    assert_eq!(
        read::<BTreeMap<String, String>>(yaml, as_text()).unwrap()["items"],
        text
    );
}

#[test]
fn null_document_into_string_is_its_text() {
    assert_eq!(read::<String>("~", as_text()).unwrap(), "~");
    assert_eq!(read::<String>("null\n", as_text()).unwrap(), "null");
    assert_eq!(read::<String>("--- \n", as_text()).unwrap(), "");
    // A stream with no documents has no scalar text.
    assert!(read::<String>("", as_text()).is_err());
}

#[test]
fn null_scalars_as_text_keeps_other_null_semantics() {
    // An explicit null tag is still not a string.
    for yaml in ["!!null ~", "!!null null", "items: !!null\n"] {
        let result = if yaml.starts_with("items") {
            read::<Field<String>>(yaml, as_text()).map(|field| field.items)
        } else {
            read::<String>(yaml, as_text())
        };
        assert!(matches!(
            result.unwrap_err().without_snippet(),
            Error::NullIntoString { .. }
        ));
    }
    // Option and untyped targets still read null as null.
    for yaml in ["items:\n", "items: ~\n", "items: null\n"] {
        assert_eq!(
            read::<Field<Option<String>>>(yaml, as_text())
                .unwrap()
                .items,
            None
        );
        assert_eq!(
            read::<Field<serde_json::Value>>(yaml, as_text())
                .unwrap()
                .items,
            serde_json::Value::Null
        );
    }
    // Quoted scalars are strings in either mode.
    assert_eq!(read::<String>("'~'", as_text()).unwrap(), "~");
    assert_eq!(read::<String>("\"\"", Options::default()).unwrap(), "");
}

#[rstest]
#[case::omitted_value("items:\n")]
#[case::alias_to_omitted_value("anchor: &empty\nitems: *empty\n")]
fn omitted_value_is_an_empty_collection(#[case] yaml: &str) {
    assert!(read::<Field<Vec<String>>>(yaml, Options::default()).is_err());
    assert!(read::<Field<BTreeMap<String, String>>>(yaml, Options::default()).is_err());
    assert!(read::<Field<Defaulted>>(yaml, Options::default()).is_err());

    assert!(
        read::<Field<Vec<String>>>(yaml, as_text())
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        read::<Field<BTreeMap<String, String>>>(yaml, as_text())
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        read::<Field<[String; 0]>>(yaml, as_text()).unwrap().items,
        [] as [String; 0]
    );
    // A struct reads as `{}`: optional and defaulted fields are filled in, required ones are missing.
    assert_eq!(
        read::<Field<Defaulted>>(yaml, as_text()).unwrap().items,
        Defaulted::default()
    );
    assert!(read::<Field<Required>>(yaml, as_text()).is_err());
    // Non-empty tuples still need their elements.
    assert!(read::<Field<(String,)>>(yaml, as_text()).is_err());
    // Option targets still read the omitted value as None.
    assert_eq!(
        read::<Field<Option<Vec<String>>>>(yaml, as_text())
            .unwrap()
            .items,
        None
    );
}

#[rstest]
#[case::document_start("---\n")]
#[case::document_start_and_end("---\n...\n")]
#[case::comment_then_document_start("# comment\n---\n")]
fn empty_document_is_an_empty_collection(#[case] yaml: &str) {
    assert!(read::<Vec<String>>(yaml, Options::default()).is_err());
    assert!(read::<BTreeMap<String, String>>(yaml, Options::default()).is_err());
    assert!(read::<Defaulted>(yaml, Options::default()).is_err());

    assert!(read::<Vec<String>>(yaml, as_text()).unwrap().is_empty());
    assert!(
        read::<BTreeMap<String, String>>(yaml, as_text())
            .unwrap()
            .is_empty()
    );
    assert_eq!(read::<EmptyTuple>(yaml, as_text()).unwrap(), EmptyTuple());
    assert_eq!(
        read::<Defaulted>(yaml, as_text()).unwrap(),
        Defaulted::default()
    );
    assert!(read::<Required>(yaml, as_text()).is_err());
}

#[rstest]
#[case::tilde("items: ~\n")]
#[case::null("items: null\n")]
#[case::tagged_null("items: !!null null\n")]
#[case::quoted_empty("items: ''\n")]
fn non_empty_or_quoted_null_is_not_a_collection(#[case] yaml: &str) {
    assert!(read::<Field<Vec<String>>>(yaml, as_text()).is_err());
    assert!(read::<Field<BTreeMap<String, String>>>(yaml, as_text()).is_err());
    assert!(read::<Field<Defaulted>>(yaml, as_text()).is_err());
}

#[rstest]
#[case::tagged_null("items: !!null\n")]
#[case::tagged_str("items: !!str\n")]
#[case::custom_tag("items: !custom\n")]
fn tagged_empty_scalar_is_not_a_collection(#[case] yaml: &str) {
    for options in [Options::default(), as_text()] {
        assert!(read::<Field<Vec<String>>>(yaml, options.clone()).is_err());
        assert!(read::<Field<BTreeMap<String, String>>>(yaml, options.clone()).is_err());
        assert!(read::<Field<Defaulted>>(yaml, options).is_err());
    }
}

#[rstest]
#[case::null("null: a\n'null': b\n", "null")]
#[case::tilde("~: a\n'~': b\n", "~")]
#[case::empty("?\n: a\n'': b\n", "")]
fn null_like_key_duplicates_its_quoted_spelling(#[case] yaml: &str, #[case] key: &str) {
    assert!(matches!(
        read::<BTreeMap<String, String>>(yaml, as_text())
            .unwrap_err()
            .without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
    for (policy, expected) in [
        (DuplicateKeyPolicy::FirstWins, "a"),
        (DuplicateKeyPolicy::LastWins, "b"),
    ] {
        let mut options = as_text();
        options.duplicate_keys = policy;
        let map = read::<BTreeMap<String, String>>(yaml, options).unwrap();
        assert_eq!(map.len(), 1, "{yaml:?}");
        assert_eq!(map[key], expected, "{yaml:?}");
    }
}

#[test]
fn distinct_null_like_keys_are_distinct_strings() {
    let map = read::<BTreeMap<String, String>>("null: a\n~: b\nNULL: c\n", as_text()).unwrap();
    assert_eq!(map.len(), 3);
    assert_eq!(map["null"], "a");
    assert_eq!(map["~"], "b");
    assert_eq!(map["NULL"], "c");
}

#[test]
fn struct_field_names_accept_null_like_keys() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct NullField {
        null: String,
    }
    assert!(read::<NullField>("null: x\n", Options::default()).is_err());
    assert_eq!(
        read::<NullField>("null: x\n", as_text()).unwrap(),
        NullField {
            null: "x".to_owned()
        }
    );
}
