#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_saphyr::{Tagged, from_str, from_str_with_options, options};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Unit {
    Null,
    Now,
}

#[rstest]
#[case("!null", Unit::Null)]
#[case("!null null", Unit::Null)]
#[case("!null ~", Unit::Null)]
#[case("!now", Unit::Now)]
#[case("!now null", Unit::Now)]
#[case("!<!null>", Unit::Null)]
#[case("%TAG !local! !\n--- !local!null", Unit::Null)]
#[case("%TAG !! !\n--- !!null", Unit::Null)]
fn local_unit_selectors_survive_options(#[case] yaml: &str, #[case] expected: Unit) {
    assert_eq!(from_str::<Option<Unit>>(yaml).unwrap(), Some(expected));
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Payload {
    Null(String),
    Const(Option<String>),
    Seq(Vec<u8>),
    Map { value: u8 },
}

#[rstest]
#[case("!null payload", Payload::Null("payload".into()))]
#[case("!null 'null'", Payload::Null("null".into()))]
#[case("!const null", Payload::Const(None))]
#[case("!const payload", Payload::Const(Some("payload".into())))]
#[case("!seq [1, 2]", Payload::Seq(vec![1, 2]))]
#[case("!map {value: 1}", Payload::Map { value: 1 })]
fn local_payloads_survive_nested_options(#[case] yaml: &str, #[case] expected: Payload) {
    assert_eq!(
        from_str::<Option<Option<Payload>>>(yaml).unwrap(),
        Some(Some(expected)),
    );
}

#[test]
fn tags_survive_both_wrapper_orders() {
    let yaml = "!null payload";
    let outer: Tagged<Option<Payload>> = from_str(yaml).unwrap();
    assert_eq!(
        outer,
        Tagged(Some(Payload::Null("payload".into())), Some("!null".into())),
    );
    let inner: Option<Tagged<Payload>> = from_str(yaml).unwrap();
    assert_eq!(
        inner,
        Some(Tagged(
            Payload::Null("payload".into()),
            Some("!null".into())
        )),
    );
}

#[rstest]
#[case("null")]
#[case("~")]
#[case("")]
#[case("!!null")]
#[case("!!null null")]
#[case("!<tag:yaml.org,2002:null> null")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !null null")]
fn ordinary_and_global_null_remain_none(#[case] yaml: &str) {
    assert_eq!(from_str::<Option<Payload>>(yaml).unwrap(), None);
}

#[rstest]
#[case("!null payload")]
#[case("!now payload")]
fn optional_unit_selectors_reject_non_null_payloads(#[case] yaml: &str) {
    assert!(from_str::<Option<Unit>>(yaml).is_err());
}

#[test]
fn inner_validation_and_strict_tag_eligibility_still_apply() {
    assert!(
        from_str_with_options::<Option<Payload>>("!null 42", options! { no_schema: true }).is_err(),
    );
    assert_eq!(
        from_str_with_options::<Option<Payload>>("!null '42'", options! { no_schema: true })
            .unwrap(),
        Some(Payload::Null("42".into())),
    );
    assert_eq!(
        from_str_with_options::<Option<Unit>>("!null", options! { reject_unsupported_tags: true },)
            .unwrap(),
        Some(Unit::Null),
    );
    assert!(
        from_str_with_options::<Option<Unit>>("!now", options! { reject_unsupported_tags: true },)
            .is_err(),
    );
}

#[test]
fn local_null_is_present_for_non_enum_options_too() {
    assert_eq!(from_str::<Option<()>>("!null").unwrap(), Some(()));
    assert!(from_str::<Option<u8>>("!null 42").is_err());
    assert_eq!(from_str::<Option<u8>>("!!null").unwrap(), None);
    assert_eq!(
        from_str::<Option<String>>("! null").unwrap(),
        Some("null".into()),
    );
}

#[rstest]
#[case("!app:kind null")]
#[case("!<!app!kind> null")]
fn non_selector_local_tags_are_present_values(#[case] yaml: &str) {
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), Some(()));
}

#[test]
fn anchored_local_selectors_survive_replay() {
    let yaml = "first: &value !null payload\nsecond: *value\n";
    let values: BTreeMap<String, Option<Payload>> = from_str(yaml).unwrap();
    for key in ["first", "second"] {
        assert_eq!(values[key], Some(Payload::Null("payload".into())));
    }
}

#[cfg(feature = "serialize")]
#[rstest]
#[case("!null", Unit::Null)]
#[case("!now", Unit::Now)]
fn optional_tagged_unit_variants_round_trip(#[case] tag: &str, #[case] variant: Unit) {
    let value = Tagged(Some(variant), Some(tag.into()));
    let yaml = serde_saphyr::to_string(&value).unwrap();
    assert_eq!(from_str::<Tagged<Option<Unit>>>(&yaml).unwrap(), value);
}

#[cfg(feature = "serialize")]
#[rstest]
#[case("!null", Payload::Null("payload".into()))]
#[case("!const", Payload::Const(None))]
fn optional_tagged_payloads_round_trip_in_both_wrapper_orders(
    #[case] tag: &str,
    #[case] variant: Payload,
) {
    let outer = Tagged(Some(variant), Some(tag.into()));
    let yaml = serde_saphyr::to_string(&outer).unwrap();
    assert_eq!(from_str::<Tagged<Option<Payload>>>(&yaml).unwrap(), outer);

    let inner = outer.0.map(|variant| Tagged(variant, outer.1));
    let yaml = serde_saphyr::to_string(&inner).unwrap();
    assert_eq!(from_str::<Option<Tagged<Payload>>>(&yaml).unwrap(), inner);
}
