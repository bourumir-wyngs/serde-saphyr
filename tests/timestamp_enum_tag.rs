#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::from_str;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Generator {
    Const(Option<String>),
    Timestamp,
    Now,
}

#[derive(Debug, Deserialize)]
struct Wrapper {
    generator: Generator,
}

#[rstest]
#[case("!timestamp")]
#[case("!<!timestamp>")]
#[case("%TAG !t! !\n--- !t!timestamp")]
fn local_timestamp_selects_unit_variant(#[case] yaml: &str) {
    assert_eq!(from_str::<Generator>(yaml).unwrap(), Generator::Timestamp);
    assert_eq!(
        from_str::<Option<Generator>>(yaml).unwrap(),
        Some(Generator::Timestamp)
    );
}

#[rstest]
#[case("generator: !timestamp\n", Generator::Timestamp)]
#[case("generator: !now\n", Generator::Now)]
#[case("generator: !const x\n", Generator::Const(Some("x".into())))]
fn local_tags_select_struct_field_variants(#[case] yaml: &str, #[case] expected: Generator) {
    assert_eq!(from_str::<Wrapper>(yaml).unwrap().generator, expected);
}

#[rstest]
#[case("[!timestamp, !now]")]
#[case("- !timestamp\n- !now\n")]
fn timestamp_variant_in_sequence(#[case] yaml: &str) {
    assert_eq!(
        from_str::<Vec<Generator>>(yaml).unwrap(),
        vec![Generator::Timestamp, Generator::Now]
    );
}

#[rstest]
#[case("!!timestamp")]
#[case("!<tag:yaml.org,2002:timestamp>")]
#[case("%TAG !t! tag:yaml.org,2002:\n--- !t!timestamp")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !timestamp")]
fn global_timestamp_does_not_select_variant(#[case] yaml: &str) {
    assert!(from_str::<Generator>(yaml).is_err());
}

#[derive(Debug, Deserialize, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
enum Payload {
    #[serde(rename = "timestamp")]
    Timestamp(String),
}

#[rstest]
#[case("!timestamp 2001-12-14", "2001-12-14")]
#[case("!timestamp arbitrary text", "arbitrary text")]
fn timestamp_selects_scalar_newtype(#[case] yaml: &str, #[case] expected: &str) {
    assert_eq!(
        from_str::<Payload>(yaml).unwrap(),
        Payload::Timestamp(expected.into())
    );
}

#[test]
fn timestamp_unit_rejects_nonempty_payload() {
    assert!(from_str::<Generator>("!timestamp value").is_err());
}

#[test]
fn timestamp_keeps_type_meaning_outside_enum() {
    assert!(from_str::<String>("!timestamp 2001-12-14").is_err());
}

#[test]
fn no_schema_validates_the_selected_payload_type() {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Number {
        #[serde(rename = "timestamp")]
        Timestamp(u32),
    }

    let options = serde_saphyr::options! { no_schema: true };
    assert_eq!(
        serde_saphyr::from_str_with_options::<Number>("!timestamp 42", options.clone()).unwrap(),
        Number::Timestamp(42)
    );
    assert!(
        serde_saphyr::from_str_with_options::<Payload>("!timestamp 42", options.clone()).is_err()
    );
    assert_eq!(
        serde_saphyr::from_str_with_options::<Payload>("!timestamp '42'", options).unwrap(),
        Payload::Timestamp("42".into())
    );
}

#[cfg(feature = "serialize")]
#[test]
fn timestamp_tag_round_trips_a_scalar_payload() {
    let value = serde_saphyr::Tagged(
        Payload::Timestamp("2001-12-14".into()),
        Some("!timestamp".into()),
    );
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(
        from_str::<serde_saphyr::Tagged<Payload>>(&yaml).unwrap(),
        value
    );
}
