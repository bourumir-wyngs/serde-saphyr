#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_saphyr::{DuplicateKeyPolicy, Error, from_str, from_str_with_options, options};
use std::collections::BTreeMap;

macro_rules! enums_with_tag_names {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Unit { $($variant),+ }

        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Scalar { $($variant(String)),+ }

        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Tuple { $($variant(u8, u8)),+ }

        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Struct { $($variant { value: u8 }),+ }

        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Sequence { $($variant(Vec<u8>)),+ }

        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Mapping { $($variant(BTreeMap<String, u8>)),+ }
    };
}

enums_with_tag_names!(Int, Float, Bool, Null, Seq, Map, Str, Timestamp, Binary);

/// `tag` selects the local spelling; the remaining parameters construct the
/// expected unit, scalar, tuple, struct, sequence, and mapping variants.
/// Returns normally when all payload shapes deserialize in both strict modes.
#[rstest]
#[case("int", Unit::Int, Scalar::Int, Tuple::Int, Struct::Int { value: 1 }, Sequence::Int, Mapping::Int)]
#[case("float", Unit::Float, Scalar::Float, Tuple::Float, Struct::Float { value: 1 }, Sequence::Float, Mapping::Float)]
#[case("bool", Unit::Bool, Scalar::Bool, Tuple::Bool, Struct::Bool { value: 1 }, Sequence::Bool, Mapping::Bool)]
#[case("null", Unit::Null, Scalar::Null, Tuple::Null, Struct::Null { value: 1 }, Sequence::Null, Mapping::Null)]
#[case("seq", Unit::Seq, Scalar::Seq, Tuple::Seq, Struct::Seq { value: 1 }, Sequence::Seq, Mapping::Seq)]
#[case("map", Unit::Map, Scalar::Map, Tuple::Map, Struct::Map { value: 1 }, Sequence::Map, Mapping::Map)]
#[case("str", Unit::Str, Scalar::Str, Tuple::Str, Struct::Str { value: 1 }, Sequence::Str, Mapping::Str)]
#[case("timestamp", Unit::Timestamp, Scalar::Timestamp, Tuple::Timestamp, Struct::Timestamp { value: 1 }, Sequence::Timestamp, Mapping::Timestamp)]
#[case("binary", Unit::Binary, Scalar::Binary, Tuple::Binary, Struct::Binary { value: 1 }, Sequence::Binary, Mapping::Binary)]
fn local_enum_tags_accept_every_payload_shape(
    #[case] tag: &str,
    #[case] unit: Unit,
    #[case] scalar: fn(String) -> Scalar,
    #[case] tuple: fn(u8, u8) -> Tuple,
    #[case] structure: Struct,
    #[case] sequence: fn(Vec<u8>) -> Sequence,
    #[case] mapping: fn(BTreeMap<String, u8>) -> Mapping,
) {
    for reject_unsupported_tags in [false, true] {
        let opts = options! { reject_unsupported_tags: reject_unsupported_tags };
        assert_eq!(
            from_str_with_options::<Unit>(&format!("!{tag}"), opts.clone()).unwrap(),
            unit,
        );
        assert_eq!(
            from_str_with_options::<Scalar>(&format!("!{tag} text"), opts.clone()).unwrap(),
            scalar("text".to_owned()),
        );
        assert_eq!(
            from_str_with_options::<Tuple>(&format!("!{tag} [1, 2]"), opts.clone()).unwrap(),
            tuple(1, 2),
        );
        assert_eq!(
            from_str_with_options::<Struct>(&format!("!{tag} {{value: 1}}"), opts.clone()).unwrap(),
            structure,
        );
        assert_eq!(
            from_str_with_options::<Sequence>(&format!("!{tag} [1, 2]"), opts.clone()).unwrap(),
            sequence(vec![1, 2]),
        );
        assert_eq!(
            from_str_with_options::<Mapping>(&format!("!{tag} {{value: 1}}"), opts).unwrap(),
            mapping(BTreeMap::from([("value".to_owned(), 1)])),
        );
    }
}

#[rstest]
#[case("!int [1]")]
#[case("!float {value: 1}")]
#[case("!bool [true]")]
#[case("!null {value: null}")]
#[case("!timestamp [2026-01-01]")]
#[case("!binary {value: SGVsbG8=}")]
#[case("!str [value]")]
#[case("!seq scalar")]
#[case("!seq {value: 1}")]
#[case("!map scalar")]
#[case("!map [1]")]
fn non_enum_consumers_still_validate_local_tag_shapes(
    #[case] yaml: &str,
    #[values(false, true)] reject_unsupported_tags: bool,
) {
    let opts = options! { reject_unsupported_tags: reject_unsupported_tags };
    let error = from_str_with_options::<IgnoredAny>(yaml, opts.clone()).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));

    #[derive(Debug, Deserialize)]
    struct KnownOnly {
        #[allow(dead_code)]
        known: u8,
    }

    let error = from_str_with_options::<KnownOnly>(&format!("known: 1\nignored: {yaml}\n"), opts)
        .unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, vec![1, 2])]
#[case(DuplicateKeyPolicy::LastWins, vec![3, 4])]
fn duplicate_values_with_local_enum_tags_keep_the_selected_value(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Vec<u8>,
) {
    // A discarded value has no target type. It may be a valid enum payload, so local
    // tag shape checks must wait until a value is actually deserialized.
    let value = from_str_with_options::<BTreeMap<String, Sequence>>(
        "value: !int [1, 2]\nvalue: !int [3, 4]\n",
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap();
    assert_eq!(
        value,
        BTreeMap::from([("value".to_owned(), Sequence::Int(expected))])
    );
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, vec![1, 2])]
#[case(DuplicateKeyPolicy::LastWins, vec![3, 4])]
fn numeric_duplicate_keys_keep_local_enum_payloads_during_buffering(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Vec<u8>,
) {
    let value = from_str_with_options::<BTreeMap<u8, Sequence>>(
        "0x1: !int [1, 2]\n1: !int [3, 4]\n",
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap();
    assert_eq!(value, BTreeMap::from([(1, Sequence::Int(expected))]));
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, "value: !int [1, 2]\nvalue: !!int []\n")]
#[case(DuplicateKeyPolicy::LastWins, "value: !!int []\nvalue: !int [1, 2]\n")]
fn discarded_duplicate_values_still_validate_global_tag_shapes(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] yaml: &str,
) {
    let error = from_str_with_options::<BTreeMap<String, Sequence>>(
        yaml,
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[rstest]
#[case("!!int [1, 2]")]
#[case("!<tag:yaml.org,2002:int> [1, 2]")]
#[case("%TAG ! tag:yaml.org,2002:\n--- !int [1, 2]")]
#[case("%TAG !core! tag:yaml.org,2002:\n--- !core!int [1, 2]")]
fn global_tags_keep_their_node_kind_requirements(#[case] yaml: &str) {
    let error = from_str::<Tuple>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[rstest]
#[case("!<!int> [1, 2]")]
#[case("%TAG !local! !\n--- !local!int [1, 2]")]
#[case("%TAG !! !\n--- !!int [1, 2]")]
fn resolved_local_tags_select_variants(#[case] yaml: &str) {
    assert_eq!(from_str::<Tuple>(yaml).unwrap(), Tuple::Int(1, 2));
}

#[test]
fn local_tags_select_variants_after_anchor_replay() {
    assert_eq!(
        from_str::<Vec<Sequence>>("[&first !int [1, 2], *first]").unwrap(),
        [Sequence::Int(vec![1, 2]), Sequence::Int(vec![1, 2])],
    );
}

#[rstest]
#[case("time")]
#[case("degrees")]
#[case("radians")]
fn local_extension_tags_select_variants_with_collection_payloads(#[case] tag: &str) {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum Extension {
        Time(Vec<u8>),
        Degrees(Vec<u8>),
        Radians(Vec<u8>),
    }

    let expected = match tag {
        "time" => Extension::Time(vec![1, 2]),
        "degrees" => Extension::Degrees(vec![1, 2]),
        "radians" => Extension::Radians(vec![1, 2]),
        _ => unreachable!(),
    };
    assert_eq!(
        from_str::<Extension>(&format!("!{tag} [1, 2]")).unwrap(),
        expected,
    );
}
