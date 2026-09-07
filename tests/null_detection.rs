#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde_json::Value;
use std::collections::BTreeMap;

#[rstest]
#[case::tagged_null("!!str null", "null")]
#[case::tagged_title_case_null("!!str Null", "Null")]
#[case::tagged_uppercase_null("!!str NULL", "NULL")]
#[case::tagged_tilde("!!str ~", "~")]
#[case::tagged_empty("!!str", "")]
#[case::non_specific_null("! null", "null")]
#[case::non_specific_title_case_null("! Null", "Null")]
#[case::non_specific_uppercase_null("! NULL", "NULL")]
#[case::non_specific_tilde("! ~", "~")]
#[case::non_specific_empty("!", "")]
#[case::verbatim_string_tag("!<tag:yaml.org,2002:str> null", "null")]
#[case::single_quoted_null("'null'", "null")]
#[case::double_quoted_null("\"null\"", "null")]
#[case::single_quoted_tilde("'~'", "~")]
#[case::double_quoted_tilde("\"~\"", "~")]
#[case::single_quoted_empty("''", "")]
#[case::double_quoted_empty("\"\"", "")]
#[case::literal_empty("|\n", "")]
#[case::literal_strip_empty("|-\n", "")]
#[case::literal_keep_empty("|+\n", "")]
#[case::folded_empty(">\n", "")]
#[case::folded_strip_empty(">-\n", "")]
#[case::folded_keep_empty(">+\n", "")]
#[case::literal_null("|-\n  null\n", "null")]
#[case::folded_null(">-\n  null\n", "null")]
fn null_like_strings_preserve_their_values(#[case] yaml: &str, #[case] expected: &str) {
    let option = serde_saphyr::from_str::<Option<String>>(yaml).unwrap();
    let json = serde_saphyr::from_str::<Value>(yaml).unwrap();

    assert_eq!(
        (option, json),
        (
            Some(expected.to_owned()),
            Value::String(expected.to_owned())
        )
    );
}

#[test]
fn non_specific_null_strings_support_borrowed_strings_and_chars() {
    for no_schema in [false, true] {
        let options = serde_saphyr::options! { no_schema: no_schema };
        for (yaml, expected) in [("! null", "null"), ("! ~", "~")] {
            let actual: Option<&str> =
                serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
            assert_eq!(actual, Some(expected), "no_schema={no_schema}: {yaml}");
        }

        let actual: char = serde_saphyr::from_str_with_options("! ~", options).unwrap();
        assert_eq!(actual, '~');
    }
}

#[test]
fn no_schema_preserves_explicit_string_and_null_tags() {
    for (yaml, expected) in [
        ("! null", Value::String("null".to_owned())),
        ("! ~", Value::String("~".to_owned())),
        ("!", Value::String(String::new())),
        ("!!str null", Value::String("null".to_owned())),
        ("!!null null", Value::Null),
    ] {
        let options = serde_saphyr::options! { no_schema: true };
        let actual: Value = serde_saphyr::from_str_with_options(yaml, options).unwrap();
        assert_eq!(actual, expected, "{yaml}");
    }
}

#[rstest]
#[case::plain_null("null")]
#[case::title_case_null("Null")]
#[case::uppercase_null("NULL")]
#[case::tilde("~")]
#[case::empty("---\n")]
#[case::tagged_empty("!!null")]
#[case::tagged_null("!!null null")]
#[case::tagged_tilde("!!null ~")]
fn null_scalars_remain_null(#[case] yaml: &str) {
    assert_eq!(
        serde_saphyr::from_str::<Option<String>>(yaml).unwrap(),
        None
    );
    assert_eq!(serde_saphyr::from_str::<Value>(yaml).unwrap(), Value::Null);
}

#[test]
fn anchored_null_like_strings_preserve_mapping_values() {
    let yaml = "\
tagged: &tagged !!str null\n\
tagged_alias: *tagged\n\
non_specific: &non_specific ! null\n\
non_specific_alias: *non_specific\n\
literal: &literal |\n\
literal_alias: *literal\n\
folded: &folded >\n\
folded_alias: *folded\n\
absent:\n";
    let options: BTreeMap<String, Option<String>> = serde_saphyr::from_str(yaml).unwrap();
    let json: Value = serde_saphyr::from_str(yaml).unwrap();

    for (key, expected) in [
        ("tagged", "null"),
        ("tagged_alias", "null"),
        ("non_specific", "null"),
        ("non_specific_alias", "null"),
        ("literal", ""),
        ("literal_alias", ""),
        ("folded", ""),
        ("folded_alias", ""),
    ] {
        assert_eq!(options[key].as_deref(), Some(expected), "key: {key}");
        assert_eq!(json[key], Value::String(expected.to_owned()), "key: {key}");
    }
    assert_eq!(options["absent"], None);
    assert_eq!(json["absent"], Value::Null);
}

#[test]
fn non_specific_null_strings_preserve_flattened_fields() {
    #[derive(serde::Deserialize)]
    struct Document {
        #[serde(flatten)]
        values: BTreeMap<String, Option<String>>,
    }

    let yaml = "null_text: ! null\ntilde_text: ! ~\nempty_text: !\nabsent:\n";
    let document: Document = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(
        document.values,
        BTreeMap::from([
            ("null_text".to_owned(), Some("null".to_owned())),
            ("tilde_text".to_owned(), Some("~".to_owned())),
            ("empty_text".to_owned(), Some(String::new())),
            ("absent".to_owned(), None),
        ])
    );
}

#[test]
fn non_specific_null_strings_are_retained_in_document_streams() {
    let yaml = "--- ! null\n--- ! ~\n--- !\n--- null\n--- kept\n";
    let expected = vec![
        "null".to_owned(),
        "~".to_owned(),
        String::new(),
        "kept".to_owned(),
    ];
    let multiple: Vec<String> = serde_saphyr::from_multiple(yaml).unwrap();
    let mut reader = yaml.as_bytes();
    let streamed: Vec<String> = serde_saphyr::read::<_, String>(&mut reader)
        .collect::<Result<_, _>>()
        .unwrap();

    assert_eq!((multiple, streamed), (expected.clone(), expected));
}

#[test]
#[cfg(feature = "serialize")]
fn optional_empty_literal_string_round_trips() {
    let expected = Some(serde_saphyr::LitString(String::new()));
    let yaml = serde_saphyr::to_string(&expected).unwrap();
    let actual: Option<serde_saphyr::LitString> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(actual, expected, "yaml: {yaml:?}");
}

#[test]
#[cfg(feature = "serialize")]
fn optional_empty_folded_string_round_trips() {
    let expected = Some(serde_saphyr::FoldString(String::new()));
    let yaml = serde_saphyr::to_string(&expected).unwrap();
    let actual: Option<serde_saphyr::FoldString> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(actual, expected, "yaml: {yaml:?}");
}
