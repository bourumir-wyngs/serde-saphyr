#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{Tagged, from_str, from_str_with_options, options};

/// `yaml` is the input, `expected` its generic value, and `strict` selects tag rejection.
/// Returns normally when the bare tag retains its standard non-specific meaning.
#[rstest]
#[case("! 123", json!("123"))]
#[case("! true", json!("true"))]
#[case("! null", json!("null"))]
#[case("! ~", json!("~"))]
#[case("!", json!(""))]
#[case("! [123, true, ! 123]", json!([123, true, "123"]))]
#[case("! {number: 123, text: ! 123}", json!({"number": 123, "text": "123"}))]
#[case("%TAG ! tag:example.org,2026:\n--- ! 123", json!("123"))]
#[case("%TAG ! tag:yaml.org,2002:\n--- ! 123", json!("123"))]
fn bare_tag_resolves_by_node_kind(
    #[case] yaml: &str,
    #[case] expected: Value,
    #[values(false, true)] strict: bool,
) {
    let value: Value =
        from_str_with_options(yaml, options! { reject_unsupported_tags: strict }).unwrap();
    assert_eq!(value, expected);
}

#[rstest]
#[case("!str 123", json!(123))]
#[case("!!str 123", json!("123"))]
#[case("! 123", json!("123"))]
#[case("!int '123'", json!("123"))]
#[case("!!int '123'", json!(123))]
#[case("!null 123", json!(123))]
#[case("!<tag:yaml.org,2002:!str> 123", json!(123))]
#[case("%TAG ! tag:yaml.org,2002:\n--- !str 123", json!("123"))]
#[case("%TAG !! !\n--- !!str 123", json!(123))]
fn local_standard_and_non_specific_tags_are_distinct(#[case] yaml: &str, #[case] expected: Value) {
    assert_eq!(from_str::<Value>(yaml).unwrap(), expected);
}

#[rstest]
#[case("!", "")]
#[case("! null", "null")]
#[case("! ~", "~")]
fn bare_tag_preserves_null_like_text(#[case] yaml: &str, #[case] expected: &str) {
    assert_eq!(
        from_str::<Option<String>>(yaml).unwrap().as_deref(),
        Some(expected)
    );
    let values: Vec<String> = serde_saphyr::from_multiple(yaml).unwrap();
    assert_eq!(values, [expected]);
}

#[test]
fn bare_tag_is_preserved_but_never_selects_a_named_variant() {
    #[derive(Debug, Deserialize)]
    enum Choice {
        #[serde(rename = "!")]
        Bang(#[allow(dead_code)] u8),
    }
    assert!(from_str::<Choice>("! 12").is_err());
    let value: Tagged<Value> = from_str("! 12").unwrap();
    assert_eq!(value, Tagged(json!("12"), Some("!".into())));
}

#[test]
fn secondary_handle_requires_a_suffix() {
    assert!(from_str::<Value>("!! 12").is_err());
}
