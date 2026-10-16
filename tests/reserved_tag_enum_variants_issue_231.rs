#![cfg(feature = "deserialize")]

//! Regression tests for https://github.com/bourumir-wyngs/serde-saphyr/issues/231.
//! Local tags that also name YAML types (`!timestamp`, `!str`, ...) select an
//! externally tagged enum variant of the same name, like any other local tag.
//! Core schema forms (`!!timestamp`) remain type annotations, and the tags keep
//! their type meaning outside enums.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Unit {
    Timestamp,
    Time,
    Str,
    Binary,
    Null,
    Now,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Payload {
    Timestamp(String),
    Str(u32),
    Seq(Vec<u32>),
    Map(BTreeMap<String, u32>),
}

#[derive(Debug, PartialEq, Deserialize)]
struct Wrapper<T> {
    value: T,
}

fn parse<T: DeserializeOwned>(yaml: &str) -> Result<T, serde_saphyr::Error> {
    serde_saphyr::from_str(yaml)
}

#[test]
fn reserved_local_tags_select_unit_variants() {
    for (yaml, expected) in [
        ("value: !timestamp\n", Unit::Timestamp),
        ("value: !time\n", Unit::Time),
        ("value: !str\n", Unit::Str),
        ("value: !binary\n", Unit::Binary),
        ("value: !null\n", Unit::Null),
        ("value: !now\n", Unit::Now),
    ] {
        let parsed: Wrapper<Unit> = parse(yaml).unwrap_or_else(|e| panic!("{yaml:?}: {e}"));
        assert_eq!(parsed.value, expected, "{yaml:?}");
    }
}

#[test]
fn reserved_local_tags_select_variants_inside_sequences() {
    let parsed: Wrapper<Vec<Unit>> = parse("value: [!timestamp, !now, !str]\n").unwrap();
    assert_eq!(parsed.value, vec![Unit::Timestamp, Unit::Now, Unit::Str]);

    let parsed: Wrapper<Vec<Unit>> = parse("value:\n  - !timestamp\n  - !binary\n").unwrap();
    assert_eq!(parsed.value, vec![Unit::Timestamp, Unit::Binary]);
}

#[test]
fn reserved_local_tags_select_variants_with_payloads() {
    let parsed: Wrapper<Payload> = parse("value: !timestamp 2001-12-14\n").unwrap();
    assert_eq!(parsed.value, Payload::Timestamp("2001-12-14".to_owned()));

    let parsed: Wrapper<Payload> = parse("value: !str 42\n").unwrap();
    assert_eq!(parsed.value, Payload::Str(42));

    let parsed: Wrapper<Payload> = parse("value: !seq [1, 2]\n").unwrap();
    assert_eq!(parsed.value, Payload::Seq(vec![1, 2]));

    let parsed: Wrapper<Payload> = parse("value: !map {a: 1}\n").unwrap();
    assert_eq!(
        parsed.value,
        Payload::Map(BTreeMap::from([("a".to_owned(), 1)]))
    );
}

#[test]
fn verbatim_local_tags_select_variants() {
    // `!<!timestamp>` is the verbatim spelling of the same local tag.
    let parsed: Wrapper<Unit> = parse("value: !<!timestamp>\n").unwrap();
    assert_eq!(parsed.value, Unit::Timestamp);
}

#[test]
fn core_schema_tags_are_not_variant_selectors() {
    assert!(parse::<Wrapper<Unit>>("value: !!timestamp\n").is_err());
    assert!(parse::<Wrapper<Unit>>("value: !<tag:yaml.org,2002:timestamp>\n").is_err());
}

#[test]
fn reserved_local_tags_keep_their_type_meaning_outside_enums() {
    let parsed: Wrapper<String> = parse("value: !str 123\n").unwrap();
    assert_eq!(parsed.value, "123");

    assert!(parse::<Wrapper<String>>("value: !timestamp 2001-12-14\n").is_err());
}
