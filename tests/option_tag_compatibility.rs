#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::{Error, NullableTilde, Tagged, from_str, from_str_with_options, options};

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[serde(rename_all = "lowercase")]
enum Choice {
    Null(String),
    Now,
    Timestamp,
    Count(u32),
    Maybe(Option<u32>),
    List(Vec<u8>),
}

/// `yaml` supplies the tagged node, `expected` its enum value, and `no_schema` the mode.
/// Returns normally when outer options preserve both the selector and payload semantics.
#[rstest]
#[case("!now", Choice::Now)]
#[case("!now null", Choice::Now)]
#[case("!now Null", Choice::Now)]
#[case("!now NULL", Choice::Now)]
#[case("!now ~", Choice::Now)]
#[case("!timestamp", Choice::Timestamp)]
#[case("!<!timestamp>", Choice::Timestamp)]
#[case("%TAG !local! !\n--- !local!timestamp", Choice::Timestamp)]
#[case("%TAG !! !\n--- !!timestamp", Choice::Timestamp)]
#[case("!null payload", Choice::Null("payload".into()))]
#[case("!null 'null'", Choice::Null("null".into()))]
#[case("!count 42", Choice::Count(42))]
#[case("!maybe", Choice::Maybe(None))]
#[case("!maybe null", Choice::Maybe(None))]
#[case("!maybe ~", Choice::Maybe(None))]
#[case("!maybe 42", Choice::Maybe(Some(42)))]
#[case("!list [1, 2]", Choice::List(vec![1, 2]))]
#[case("{now: null}", Choice::Now)]
#[case("{maybe: null}", Choice::Maybe(None))]
fn optional_values_preserve_enum_selectors(
    #[case] yaml: &str,
    #[case] expected: Choice,
    #[values(false, true)] no_schema: bool,
) {
    let opts = options! { no_schema: no_schema };
    assert_eq!(
        from_str_with_options::<Choice>(yaml, opts.clone()).unwrap(),
        expected
    );
    assert_eq!(
        from_str_with_options::<Option<Choice>>(yaml, opts.clone()).unwrap(),
        Some(expected),
    );
    let direct = from_str_with_options::<Choice>(yaml, opts.clone()).unwrap();
    assert_eq!(
        from_str_with_options::<Option<Option<Choice>>>(yaml, opts).unwrap(),
        Some(Some(direct)),
    );
}

#[rstest]
#[case("")]
#[case("---")]
#[case("null")]
#[case("Null")]
#[case("NULL")]
#[case("~")]
#[case("!!null")]
#[case("!!null null")]
#[case("!!null 'null'")]
#[case("!<tag:yaml.org,2002:null> ~")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !null")]
#[case("%TAG !n! tag:yaml.org,2002:nu\n--- !n!ll")]
fn actual_null_nodes_remain_none(#[case] yaml: &str) {
    assert_eq!(from_str::<Option<Choice>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<String>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<u8>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<bool>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<Vec<u8>>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), None);
}

#[rstest]
#[case("!unknown")]
#[case("!unknown null")]
#[case("!count")]
#[case("!count null")]
#[case("!count ~")]
#[case("!count nope")]
#[case("!null")]
#[case("!null null")]
#[case("!!int")]
#[case("!!int null")]
#[case("!!str")]
fn optional_enums_preserve_inner_errors(#[case] yaml: &str) {
    let direct = from_str::<Choice>(yaml).unwrap_err();
    let optional = from_str::<Option<Choice>>(yaml).unwrap_err();
    assert_eq!(
        optional.without_snippet().to_string(),
        direct.without_snippet().to_string()
    );
}

#[rstest]
#[case("!null")]
#[case("!null null")]
#[case("!foo ~")]
#[case("!<tag:example.org,2026:unit> null")]
fn tagged_presence_does_not_depend_on_an_enum_target(#[case] yaml: &str) {
    from_str::<()>(yaml).unwrap();
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), Some(()));
    assert!(from_str::<u8>(yaml).is_err());
    assert!(from_str::<Option<u8>>(yaml).is_err());
    assert!(from_str::<String>(yaml).is_err());
    assert!(from_str::<Option<String>>(yaml).is_err());
}

#[test]
fn nested_fields_and_aliases_preserve_tagged_values() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        absent: Option<Choice>,
        empty: Option<Choice>,
        value: Option<Choice>,
        payload: Option<Choice>,
    }
    let value: Config = from_str("empty:\nvalue: !timestamp\npayload: !maybe\n").unwrap();
    assert_eq!(
        value,
        Config {
            absent: None,
            empty: None,
            value: Some(Choice::Timestamp),
            payload: Some(Choice::Maybe(None)),
        }
    );
    assert_eq!(
        from_str::<Vec<Option<Choice>>>("[&value !timestamp, *value]").unwrap(),
        [Some(Choice::Timestamp), Some(Choice::Timestamp)],
    );
}

#[test]
fn optional_enum_keys_preserve_selectors() {
    use std::collections::BTreeMap;
    let value: BTreeMap<Option<Choice>, u8> = from_str("!now : 1\n!maybe : 2\n").unwrap();
    assert_eq!(
        value,
        BTreeMap::from([(Some(Choice::Now), 1), (Some(Choice::Maybe(None)), 2),])
    );
}

#[rstest]
#[case("!timestamp")]
#[case("!timestamp null")]
fn wrappers_preserve_present_enum_values(#[case] yaml: &str) {
    assert_eq!(
        from_str::<NullableTilde<Choice>>(yaml).unwrap(),
        NullableTilde(Some(Choice::Timestamp)),
    );
    assert_eq!(
        from_str::<Tagged<Option<Choice>>>(yaml).unwrap(),
        Tagged(Some(Choice::Timestamp), Some("!timestamp".into())),
    );
    assert_eq!(
        from_str::<Option<Tagged<Choice>>>(yaml).unwrap(),
        Some(Tagged(Choice::Timestamp, Some("!timestamp".into()))),
    );
}

#[test]
fn strict_tag_rejection_is_unchanged() {
    let opts = options! { reject_unsupported_tags: true };
    let error = from_str_with_options::<Option<Choice>>("!timestamp", opts).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[cfg(feature = "serialize")]
#[rstest]
#[case(Choice::Timestamp)]
#[case(Choice::Maybe(None))]
#[case(Choice::Maybe(Some(42)))]
fn present_optional_enums_round_trip(#[case] expected: Choice) {
    let value = Some(expected);
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(from_str::<Option<Choice>>(&yaml).unwrap(), value);
}

#[cfg(feature = "serialize")]
#[test]
fn tagged_optional_enum_round_trips() {
    let value = Tagged(Some(Choice::Timestamp), Some("!timestamp".into()));
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(from_str::<Tagged<Option<Choice>>>(&yaml).unwrap(), value);
}

#[cfg(feature = "serialize")]
#[rstest]
#[case("!null")]
#[case("!now")]
#[case("!foo")]
fn application_tags_do_not_encode_absent_values(#[case] tag: &str) {
    let value = Tagged(None::<String>, Some(tag.into()));
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(yaml, format!("{tag} null\n"));
    // The tag denotes a present value. Its null payload is not a valid String.
    assert!(from_str::<Tagged<Option<String>>>(&yaml).is_err());
    assert_eq!(
        from_str::<Tagged<Option<()>>>(&yaml).unwrap(),
        Tagged(Some(()), Some(tag.into())),
    );
    let tilde = Tagged(NullableTilde::<String>(None), Some(tag.into()));
    let yaml = serde_saphyr::to_string(&tilde).unwrap();
    assert_eq!(yaml, format!("{tag} ~\n"));
    assert!(from_str::<Tagged<NullableTilde<String>>>(&yaml).is_err());
}
