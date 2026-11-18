#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, from_str, from_str_with_options, options};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Generator {
    Const(Option<String>),
    Timestamp,
    Now,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Wrapper {
    generator: Generator,
}

#[rstest]
#[case("generator: !timestamp", Generator::Timestamp)]
#[case("generator: !now", Generator::Now)]
#[case("generator: !const text", Generator::Const(Some("text".into())))]
#[case("generator: !const", Generator::Const(None))]
fn original_issue_selects_the_variant(#[case] yaml: &str, #[case] expected: Generator) {
    assert_eq!(from_str::<Wrapper>(yaml).unwrap().generator, expected);
}

#[rstest]
#[case("!timestamp")]
#[case("!<!timestamp>")]
#[case("%TAG !local! !\n--- !local!timestamp")]
#[case("%TAG !! !\n--- !!timestamp")]
fn resolved_local_timestamp_is_a_unit_selector(#[case] yaml: &str) {
    assert_eq!(from_str::<Generator>(yaml).unwrap(), Generator::Timestamp);
}

#[rstest]
#[case("!!timestamp")]
#[case("!<tag:yaml.org,2002:timestamp>")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !timestamp")]
#[case("%TAG !core! tag:yaml.org,2002:\n--- !core!timestamp")]
fn resolved_global_timestamp_is_not_a_variant_selector(#[case] yaml: &str) {
    assert!(from_str::<Generator>(yaml).is_err());
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum NumericPayload {
    Int(u32),
    Float(u32),
    Bool(u32),
    Null(u32),
    Str(u32),
    Seq(u32),
    Map(u32),
    Timestamp(u32),
    Binary(u32),
    Time(u32),
    Degrees(u32),
    Radians(u32),
}

#[rstest]
#[case("!int 42", NumericPayload::Int(42))]
#[case("!float 42", NumericPayload::Float(42))]
#[case("!bool 42", NumericPayload::Bool(42))]
#[case("!null 42", NumericPayload::Null(42))]
#[case("!str 42", NumericPayload::Str(42))]
#[case("!seq 42", NumericPayload::Seq(42))]
#[case("!map 42", NumericPayload::Map(42))]
#[case("!timestamp 42", NumericPayload::Timestamp(42))]
#[case("!binary 42", NumericPayload::Binary(42))]
#[case("!time 42", NumericPayload::Time(42))]
#[case("!degrees 42", NumericPayload::Degrees(42))]
#[case("!radians 42", NumericPayload::Radians(42))]
fn selector_precedes_scalar_validation_and_no_schema(
    #[case] yaml: &str,
    #[case] expected: NumericPayload,
) {
    assert_eq!(
        from_str_with_options::<NumericPayload>(yaml, options! { no_schema: true }).unwrap(),
        expected,
    );
}

#[test]
fn no_schema_still_checks_the_selected_string_payload() {
    #[derive(Debug, Deserialize)]
    enum Text {
        #[serde(rename = "timestamp")]
        Timestamp(#[allow(dead_code)] String),
    }

    let error =
        from_str_with_options::<Text>("!timestamp 42", options! { no_schema: true }).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::QuotingRequired { .. }
    ));
    assert!(from_str_with_options::<Text>("!timestamp '42'", options! { no_schema: true }).is_ok());
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum StringSelector {
    Str(String),
    Timestamp,
}

#[test]
fn matching_local_string_tag_selects_a_variant_before_reading_its_payload() {
    assert_eq!(
        from_str::<StringSelector>("!str timestamp").unwrap(),
        StringSelector::Str("timestamp".to_owned()),
    );
    assert_eq!(
        from_str::<StringSelector>("!!str timestamp").unwrap(),
        StringSelector::Timestamp,
    );
}

#[test]
fn unmatched_local_string_tag_does_not_force_a_string() {
    assert!(from_str::<Generator>("!str timestamp").is_err());
}

#[rstest]
#[case("!int timestamp")]
#[case("!bool timestamp")]
#[case("!seq timestamp")]
#[case("!map timestamp")]
fn unmatched_local_tags_do_not_bypass_validation(#[case] yaml: &str) {
    assert!(from_str::<Generator>(yaml).is_err());
}

#[test]
fn unit_selector_still_rejects_a_non_null_payload() {
    assert!(from_str::<Generator>("!timestamp not-empty").is_err());
}

#[test]
fn wrappers_preserve_the_typed_enum_context() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Newtype(Generator);

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(transparent)]
    struct Transparent {
        inner: Newtype,
    }

    assert_eq!(
        from_str::<Transparent>("!timestamp").unwrap(),
        Transparent {
            inner: Newtype(Generator::Timestamp),
        },
    );
}
