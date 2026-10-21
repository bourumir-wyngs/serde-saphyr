#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_saphyr::from_str;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Choice {
    Null(String),
    Now,
    Timestamp,
}

#[rstest]
#[case("!null payload", Choice::Null("payload".into()))]
#[case("!null 'null'", Choice::Null("null".into()))]
#[case("!now", Choice::Now)]
#[case("!now null", Choice::Now)]
#[case("!now ~", Choice::Now)]
#[case("!<!now>", Choice::Now)]
fn optional_enums_retain_existing_null_resolution(#[case] yaml: &str, #[case] direct: Choice) {
    // The selector works directly, but Option consumes null before asking
    // the inner enum to deserialize. This limitation preserves compatibility.
    assert_eq!(from_str::<Choice>(yaml).unwrap(), direct);
    assert_eq!(from_str::<Option<Choice>>(yaml).unwrap(), None);
}

#[rstest]
#[case("!null")]
#[case("!null 42")]
#[case("!!null")]
fn ordinary_optional_types_keep_null_alias_compatibility(#[case] yaml: &str) {
    assert_eq!(from_str::<Option<String>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<u8>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<bool>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<Vec<u8>>>(yaml).unwrap(), None);
    assert_eq!(from_str::<Option<()>>(yaml).unwrap(), None);
}

#[rstest]
#[case("{'null': payload}", Choice::Null("payload".into()))]
#[case("{now: null}", Choice::Now)]
#[case("!timestamp", Choice::Timestamp)]
fn optional_enums_support_mappings_and_non_null_tags(#[case] yaml: &str, #[case] expected: Choice) {
    assert_eq!(from_str::<Option<Choice>>(yaml).unwrap(), Some(expected));
}

#[cfg(feature = "serialize")]
#[rstest]
#[case("!null")]
#[case("!now")]
#[case("!foo")]
fn locally_tagged_none_remains_serializable(#[case] tag: &str) {
    use serde_saphyr::{NullableTilde, Tagged, to_string};

    let value = Tagged(None::<String>, Some(tag.into()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, format!("{tag} null\n"));
    assert_eq!(from_str::<Tagged<Option<String>>>(&yaml).unwrap(), value);

    let tilde = Tagged(NullableTilde::<String>(None), Some(tag.into()));
    let yaml = to_string(&tilde).unwrap();
    assert_eq!(yaml, format!("{tag} ~\n"));
    assert_eq!(
        from_str::<Tagged<NullableTilde<String>>>(&yaml).unwrap(),
        tilde
    );
}
