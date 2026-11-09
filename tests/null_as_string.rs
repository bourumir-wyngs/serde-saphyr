#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::{Error, Options, from_reader_with_options, from_str_with_options};
use std::borrow::Cow;
use std::collections::BTreeMap;

const NULL_TEXTS: [&str; 5] = ["null", "Null", "NULL", "~", ""];

fn enabled() -> Options {
    serde_saphyr::options! { null_as_string: true }
}

#[test]
fn null_strings_are_opt_in() {
    assert!(!Options::default().null_as_string);
    for text in NULL_TEXTS {
        let yaml = format!("--- {text}\n");
        for error in [
            serde_saphyr::from_str::<String>(&yaml).unwrap_err(),
            serde_saphyr::from_str::<&str>(&yaml).unwrap_err(),
            serde_saphyr::from_str::<Cow<'_, str>>(&yaml).unwrap_err(),
        ] {
            assert!(
                matches!(error.without_snippet(), Error::NullIntoString { .. }),
                "{text:?}: {error}"
            );
        }

        let yaml = format!("? {text}\n: 1\n");
        assert!(serde_saphyr::from_str::<BTreeMap<String, u32>>(&yaml).is_err());
        assert!(serde_saphyr::from_str::<serde_json::Value>(&yaml).is_err());
    }
}

#[test]
fn owned_borrowed_and_cow_roots_preserve_null_text() {
    for text in NULL_TEXTS {
        let yaml = format!("--- {text}\n");
        let owned: String = from_str_with_options(&yaml, enabled()).unwrap();
        let borrowed: &str = from_str_with_options(&yaml, enabled()).unwrap();
        let cow: Cow<'_, str> = from_str_with_options(&yaml, enabled()).unwrap();
        let from_slice: &str =
            serde_saphyr::from_slice_with_options(yaml.as_bytes(), enabled()).unwrap();
        let from_reader: String = from_reader_with_options(yaml.as_bytes(), enabled()).unwrap();
        let reader_cow: Cow<'_, str> =
            from_reader_with_options(yaml.as_bytes(), enabled()).unwrap();

        assert_eq!(owned, text);
        assert_eq!(borrowed, text);
        assert_eq!(cow, text);
        assert_eq!(from_slice, text);
        assert_eq!(from_reader, text);
        assert_eq!(reader_cow, text);
    }
}

#[test]
fn mapping_keys_and_values_preserve_null_text() {
    for text in NULL_TEXTS {
        let yaml = format!("? {text}\n: {text}\n");
        let owned: BTreeMap<String, String> = from_str_with_options(&yaml, enabled()).unwrap();
        let borrowed: BTreeMap<&str, &str> = from_str_with_options(&yaml, enabled()).unwrap();
        let cow: BTreeMap<Cow<'_, str>, Cow<'_, str>> =
            from_str_with_options(&yaml, enabled()).unwrap();
        let streamed: BTreeMap<String, String> =
            from_reader_with_options(yaml.as_bytes(), enabled()).unwrap();

        assert_eq!(owned, BTreeMap::from([(text.to_owned(), text.to_owned())]));
        assert_eq!(borrowed, BTreeMap::from([(text, text)]));
        assert_eq!(
            cow,
            BTreeMap::from([(Cow::Borrowed(text), Cow::Borrowed(text))])
        );
        assert_eq!(streamed, owned);

        let value: serde_json::Value = from_str_with_options(&yaml, enabled()).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 1);
        assert_eq!(value[text], serde_json::Value::Null);
    }
}

#[test]
fn struct_fields_and_borrowing_cow_preserve_null_text() {
    #[derive(Debug, Deserialize)]
    struct Strings<'a> {
        owned: String,
        borrowed: &'a str,
        #[serde(borrow)]
        cow: Cow<'a, str>,
    }

    for text in NULL_TEXTS {
        let yaml = format!("owned: {text}\nborrowed: {text}\ncow: {text}\n");
        for value in [
            from_str_with_options::<Strings<'_>>(&yaml, enabled()).unwrap(),
            serde_saphyr::from_slice_with_options::<Strings<'_>>(yaml.as_bytes(), enabled())
                .unwrap(),
        ] {
            assert_eq!(value.owned, text);
            assert_eq!(value.borrowed, text);
            assert!(matches!(value.cow, Cow::Borrowed(value) if value == text));
        }
    }
}

#[test]
fn null_string_aliases_preserve_the_anchored_text() {
    for text in NULL_TEXTS {
        let yaml = format!("first: &value {text}\nsecond: *value\n");
        let value: BTreeMap<&str, &str> = from_str_with_options(&yaml, enabled()).unwrap();
        assert_eq!(value, BTreeMap::from([("first", text), ("second", text)]));

        let value: BTreeMap<String, String> =
            from_reader_with_options(yaml.as_bytes(), enabled()).unwrap();
        assert_eq!(value["first"], text);
        assert_eq!(value["second"], text);
    }
}

#[test]
fn null_aliases_can_be_string_mapping_keys() {
    for text in NULL_TEXTS {
        let yaml = format!("original: &key {text}\n? *key\n: value\n");
        let value: BTreeMap<&str, &str> = from_str_with_options(&yaml, enabled()).unwrap();
        assert_eq!(value, BTreeMap::from([("original", text), (text, "value")]));

        let value: BTreeMap<String, String> =
            from_reader_with_options(yaml.as_bytes(), enabled()).unwrap();
        assert_eq!(value["original"], text);
        assert_eq!(value[text], "value");
    }
}

#[test]
fn explicit_null_tags_still_reject_string_targets() {
    for yaml in ["!!null null", "!!null ~", "!!null", "!!null 'null'"] {
        for error in [
            from_str_with_options::<String>(yaml, enabled()).unwrap_err(),
            from_str_with_options::<&str>(yaml, enabled()).unwrap_err(),
            from_str_with_options::<Cow<'_, str>>(yaml, enabled()).unwrap_err(),
        ] {
            assert!(
                matches!(error.without_snippet(), Error::NullIntoString { .. }),
                "{yaml}: {error}"
            );
        }

        let mapping = format!("? {yaml}\n: value\n");
        assert!(from_str_with_options::<BTreeMap<String, String>>(&mapping, enabled()).is_err());
    }
}

#[test]
fn optional_and_untyped_nulls_keep_their_meaning() {
    for text in NULL_TEXTS {
        let yaml = format!("--- {text}\n");
        assert_eq!(
            from_str_with_options::<Option<String>>(&yaml, enabled()).unwrap(),
            None
        );
        assert_eq!(
            from_str_with_options::<Option<&str>>(&yaml, enabled()).unwrap(),
            None
        );
        assert_eq!(
            from_str_with_options::<Option<Cow<'_, str>>>(&yaml, enabled()).unwrap(),
            None
        );
        assert_eq!(
            from_str_with_options::<serde_json::Value>(&yaml, enabled()).unwrap(),
            serde_json::Value::Null
        );
        from_str_with_options::<()>(&yaml, enabled()).unwrap();
        assert!(from_str_with_options::<char>(&yaml, enabled()).is_err());

        let yaml = format!("? {text}\n: 1\n");
        let value: BTreeMap<Option<String>, u32> = from_str_with_options(&yaml, enabled()).unwrap();
        assert_eq!(value, BTreeMap::from([(None, 1)]));
    }
}

#[test]
fn no_schema_still_requires_quoting() {
    for text in NULL_TEXTS {
        let yaml = format!("--- {text}\n");
        let options = serde_saphyr::options! { null_as_string: true, no_schema: true };
        for error in [
            from_str_with_options::<String>(&yaml, options.clone()).unwrap_err(),
            from_str_with_options::<&str>(&yaml, options.clone()).unwrap_err(),
            from_str_with_options::<Cow<'_, str>>(&yaml, options).unwrap_err(),
        ] {
            assert!(
                matches!(error.without_snippet(), Error::QuotingRequired { .. }),
                "{text:?}: {error}"
            );
        }
    }

    for yaml in ["'null'", "!!str null", "! null"] {
        let options = serde_saphyr::options! { null_as_string: true, no_schema: true };
        assert_eq!(
            from_str_with_options::<String>(yaml, options).unwrap(),
            "null"
        );
    }
}

#[test]
fn buffered_mappings_preserve_null_strings() {
    for policy in [
        serde_saphyr::DuplicateKeyPolicy::FirstWins,
        serde_saphyr::DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { null_as_string: true, duplicate_keys: policy };
        let value: BTreeMap<String, String> = from_str_with_options("null: ~\n", options).unwrap();
        assert_eq!(value, BTreeMap::from([("null".to_owned(), "~".to_owned())]));
    }

    for policy in [
        serde_saphyr::DuplicateKeyPolicy::Error,
        serde_saphyr::DuplicateKeyPolicy::FirstWins,
        serde_saphyr::DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { null_as_string: true, duplicate_keys: policy };
        let value: BTreeMap<String, String> =
            from_str_with_options("null: first\n~: second\n", options).unwrap();
        assert_eq!(
            value,
            BTreeMap::from([
                ("null".to_owned(), "first".to_owned()),
                ("~".to_owned(), "second".to_owned()),
            ])
        );
    }

    let error =
        from_str_with_options::<BTreeMap<String, String>>("null: first\nnull: second\n", enabled())
            .unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));

    // The parser uses the same null marker for omitted keys and literal `~`.
    // Rendering the omitted key as empty text must not change key equality.
    let yaml = "?\n: first\n~: second\n";
    let baseline = serde_saphyr::from_str::<BTreeMap<Option<()>, String>>(yaml).unwrap_err();
    let opted_in = from_str_with_options::<BTreeMap<String, String>>(yaml, enabled()).unwrap_err();
    for error in [baseline, opted_in] {
        assert!(matches!(
            error.without_snippet(),
            Error::DuplicateMappingKey { .. }
        ));
    }
}

#[test]
fn serde_buffering_still_infers_null() {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    enum StringOrNull {
        Text(String),
        Null(()),
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct FlatStrings {
        #[serde(flatten)]
        values: BTreeMap<String, String>,
    }

    #[derive(Debug, Deserialize)]
    struct FlatValues {
        #[serde(flatten)]
        values: BTreeMap<String, serde_json::Value>,
    }

    for text in NULL_TEXTS {
        let yaml = format!("--- {text}\n");
        assert_eq!(
            from_str_with_options::<StringOrNull>(&yaml, enabled()).unwrap(),
            StringOrNull::Null(())
        );

        let yaml = format!("value: {text}\n");
        assert!(from_str_with_options::<FlatStrings>(&yaml, enabled()).is_err());
        let value: FlatValues = from_str_with_options(&yaml, enabled()).unwrap();
        assert_eq!(value.values["value"], serde_json::Value::Null);
    }
}
