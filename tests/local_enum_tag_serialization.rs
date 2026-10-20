#![cfg(all(feature = "serialize", feature = "deserialize"))]

use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_saphyr::{SerializeError, Tagged, from_str, to_string};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Unit {
    Null,
    Bool,
    Int,
    Float,
    Str,
    Seq,
    Map,
    Timestamp,
    Binary,
    Time,
    Degrees,
    Radians,
}

#[rstest]
#[case("!null", Unit::Null)]
#[case("!bool", Unit::Bool)]
#[case("!int", Unit::Int)]
#[case("!float", Unit::Float)]
#[case("!str", Unit::Str)]
#[case("!seq", Unit::Seq)]
#[case("!map", Unit::Map)]
#[case("!timestamp", Unit::Timestamp)]
#[case("!binary", Unit::Binary)]
#[case("!time", Unit::Time)]
#[case("!degrees", Unit::Degrees)]
#[case("!radians", Unit::Radians)]
fn local_tags_serialize_unit_variants_with_null_payloads(#[case] tag: &str, #[case] unit: Unit) {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Wrapper {
        generator: Tagged<Unit>,
    }

    let value = Tagged(unit, Some(tag.to_owned()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, format!("{tag} null\n"));
    assert_eq!(from_str::<Tagged<Unit>>(&yaml).unwrap(), value);

    let wrapper = Wrapper { generator: value };
    let yaml = to_string(&wrapper).unwrap();
    assert_eq!(yaml, format!("generator: {tag} null\n"));
    assert_eq!(from_str::<Wrapper>(&yaml).unwrap(), wrapper);
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Payload {
    Null(String),
    Bool(String),
    Int(String),
    Float(String),
    Str(u32),
    Seq(i32),
    Map(bool),
    Timestamp(String),
    Binary(String),
    Time(String),
    Degrees(String),
    Radians(String),
    Include(String),
    Merge(String),
    Value(String),
}

#[rstest]
#[case("!null", Payload::Null("not null".into()))]
#[case("!bool", Payload::Bool("not a boolean".into()))]
#[case("!int", Payload::Int("not an integer".into()))]
#[case("!float", Payload::Float("not a float".into()))]
#[case("!str", Payload::Str(7))]
#[case("!seq", Payload::Seq(7))]
#[case("!map", Payload::Map(true))]
#[case("!timestamp", Payload::Timestamp("not a timestamp".into()))]
#[case("!binary", Payload::Binary("not base64!".into()))]
#[case("!time", Payload::Time("not a duration".into()))]
#[case("!degrees", Payload::Degrees("not an angle".into()))]
#[case("!radians", Payload::Radians("not an angle".into()))]
fn local_tags_preserve_newtype_payload_types(#[case] tag: &str, #[case] payload: Payload) {
    let value = Tagged(payload, Some(tag.to_owned()));
    let yaml = to_string(&value).unwrap();
    assert!(yaml.starts_with(&format!("{tag} ")), "{yaml}");
    assert_eq!(from_str::<Tagged<Payload>>(&yaml).unwrap(), value);
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Collection {
    Str(i32, bool),
    Int { answer: i32 },
    Null(i32, bool),
    Seq { answer: i32 },
    Map(i32, bool),
    Timestamp { answer: i32 },
}

#[rstest]
#[case("!str", Collection::Str(7, true))]
#[case("!int", Collection::Int { answer: 7 })]
#[case("!null", Collection::Null(7, true))]
#[case("!seq", Collection::Seq { answer: 7 })]
#[case("!map", Collection::Map(7, true))]
#[case("!timestamp", Collection::Timestamp { answer: 7 })]
fn local_tags_preserve_tuple_and_struct_payloads(#[case] tag: &str, #[case] payload: Collection) {
    let value = Tagged(payload, Some(tag.to_owned()));
    let yaml = to_string(&value).unwrap();
    assert!(yaml.starts_with(&format!("{tag}\n")), "{yaml}");
    assert_eq!(from_str::<Tagged<Collection>>(&yaml).unwrap(), value);
}

#[rstest]
#[case("null", Payload::Null("value".into()))]
#[case("bool", Payload::Bool("value".into()))]
#[case("int", Payload::Int("value".into()))]
#[case("float", Payload::Float("value".into()))]
#[case("str", Payload::Str(7))]
#[case("seq", Payload::Seq(7))]
#[case("map", Payload::Map(true))]
#[case("timestamp", Payload::Timestamp("value".into()))]
#[case("binary", Payload::Binary("value".into()))]
#[case("time", Payload::Time("value".into()))]
#[case("degrees", Payload::Degrees("value".into()))]
#[case("radians", Payload::Radians("value".into()))]
#[case("include", Payload::Include("value".into()))]
#[case("merge", Payload::Merge("value".into()))]
#[case("value", Payload::Value("value".into()))]
fn global_builtin_tags_cannot_select_newtype_variants(
    #[case] variant: &str,
    #[case] payload: Payload,
) {
    let tag = format!("tag:yaml.org,2002:{variant}");
    let error = to_string(&Tagged(payload, Some(tag.clone()))).unwrap_err();
    assert!(matches!(
        error,
        SerializeError::CoreTypeTagAsEnumVariant {
            tag: error_tag,
            variant: error_variant,
        } if error_tag == tag && error_variant == variant
    ));
}

#[test]
fn global_str_tag_retains_its_string_meaning_for_unit_variants() {
    let value = Tagged(Unit::Str, Some("tag:yaml.org,2002:str".to_owned()));
    let yaml = to_string(&value).unwrap();
    assert_eq!(yaml, "!<tag:yaml.org,2002:str> str\n");
    assert_eq!(from_str::<Tagged<Unit>>(&yaml).unwrap(), value);
}
