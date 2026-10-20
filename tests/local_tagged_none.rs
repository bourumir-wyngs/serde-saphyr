#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_saphyr::{NullableTilde, SerializeError, Serializer, Tagged, from_str, to_string};

#[rstest]
#[case("!null")]
#[case("!now")]
#[case("!const")]
#[case("!app:kind")]
#[case("!!null")]
fn local_tags_cannot_represent_an_absent_optional_value(#[case] tag: &str) {
    let value = Tagged(None::<String>, Some(tag.to_owned()));
    let mut output = String::new();
    let error = value
        .serialize(&mut Serializer::new(&mut output))
        .unwrap_err();
    assert!(matches!(
        &error,
        SerializeError::LocalTagOnNone { tag: actual } if actual == tag
    ));
    assert!(error.to_string().contains("cannot serialize None"));
    assert!(output.is_empty());
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum OptionalPayload {
    Const(Option<String>),
    Null(Option<String>),
}

#[rstest]
#[case("!const", OptionalPayload::Const(None))]
#[case("!null", OptionalPayload::Null(None))]
fn claimed_enum_selectors_allow_absent_variant_payloads(
    #[case] tag: &str,
    #[case] payload: OptionalPayload,
) {
    let value = Tagged(Some(payload), Some(tag.to_owned()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, format!("{tag} null\n"));
    assert_eq!(
        from_str::<Tagged<Option<OptionalPayload>>>(&yaml).unwrap(),
        value
    );
}

#[rstest]
#[case(None)]
#[case(Some("tag:yaml.org,2002:null"))]
fn ordinary_and_global_nulls_still_represent_none(#[case] tag: Option<&str>) {
    let value = Tagged(None::<String>, tag.map(str::to_owned));
    let yaml = to_string(&value).unwrap();
    assert_eq!(from_str::<Tagged<Option<String>>>(&yaml).unwrap(), value);
}

#[test]
fn nested_optional_absence_cannot_bypass_the_local_tag_check() {
    let value = Tagged(Some(None::<String>), Some("!null".to_owned()));
    assert!(matches!(
        to_string(&value).unwrap_err(),
        SerializeError::LocalTagOnNone { .. }
    ));
}

#[test]
fn nullable_tilde_cannot_bypass_the_local_tag_check() {
    let value = Tagged(NullableTilde::<String>(None), Some("!null".to_owned()));
    assert!(matches!(
        to_string(&value).unwrap_err(),
        SerializeError::LocalTagOnNone { .. }
    ));
}

#[test]
fn claimed_enum_selectors_allow_nullable_tilde_variant_payloads() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum TildePayload {
        #[serde(rename = "const")]
        Const(NullableTilde<String>),
    }

    let value = Tagged(
        Some(TildePayload::Const(NullableTilde(None))),
        Some("!const".to_owned()),
    );
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!const ~\n");
    assert_eq!(
        from_str::<Tagged<Option<TildePayload>>>(&yaml).unwrap(),
        value,
    );
}
