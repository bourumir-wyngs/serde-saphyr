#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Value, json};
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
/// Returns normally when all payload shapes deserialize in permissive mode.
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
    let opts = options! { reject_unsupported_tags: false };
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
fn ordinary_consumers_ignore_local_type_names(#[case] yaml: &str) {
    assert!(from_str::<IgnoredAny>(yaml).is_ok());
    assert!(from_str::<serde_json::Value>(yaml).is_ok());

    #[derive(Debug, Deserialize)]
    struct KnownOnly {
        #[allow(dead_code)]
        known: u8,
    }

    assert!(from_str::<KnownOnly>(&format!("known: 1\nignored: {yaml}\n")).is_ok());
}

#[test]
fn local_tags_distinguish_variants_with_the_same_value() {
    let yaml = "[!int text, !str text]";
    assert_eq!(from_str::<Value>(yaml).unwrap(), json!(["text", "text"]));

    let opts = options! { local_tags_as_maps: true };
    let value: Value = from_str_with_options(yaml, opts.clone()).unwrap();
    assert_eq!(value, json!([{"int": "text"}, {"str": "text"}]));

    let direct: Vec<Scalar> = from_str_with_options(yaml, opts).unwrap();
    assert_eq!(
        serde_json::from_value::<Vec<Scalar>>(value).unwrap(),
        direct
    );
}

#[rstest]
#[case(
    "!Batch [!Stop, !app.add 5, !Move {x: 1}]",
    json!({"Batch": [{"Stop": null}, {"app.add": 5}, {"Move": {"x": 1}}]})
)]
#[case("%TAG !v! !\n--- !v!Add 5", json!({"Add": 5}))]
#[case("!ns:op 5", json!({"ns:op": 5}))]
#[case("%TAG !v! !ns:\n--- !v!op 5", json!({"ns:op": 5}))]
#[case("!<!op!name> 5", json!({"op!name": 5}))]
#[case("%TAG ! tag:example.org,2026:\n--- !Add 5", json!(5))]
#[case("!degrees 180", json!({"degrees": 180}))]
#[case("[&val !Type 1, *val]", json!([{"Type": 1}, {"Type": 1}]))]
#[case("[!!str 5, ! 5, !!int 5, !!Unknown 5]", json!(["5", "5", 5, 5]))]
fn local_tags_as_maps_uses_resolved_tags(#[case] yaml: &str, #[case] expected: Value) {
    let opts = options! { local_tags_as_maps: true };
    assert_eq!(
        from_str_with_options::<Value>(yaml, opts).unwrap(),
        expected
    );
}

#[test]
fn local_tags_as_maps_wraps_each_document() {
    let opts = options! { local_tags_as_maps: true };
    let documents: Vec<Value> =
        serde_saphyr::from_multiple_with_options("--- !Stop\n--- !Add 5\n--- 6\n", opts).unwrap();
    assert_eq!(
        documents,
        [json!({"Stop": null}), json!({"Add": 5}), json!(6)]
    );
}

#[rstest]
#[case(false, json!(5))]
#[case(true, json!({"T": 5}))]
fn tagged_value_preserves_metadata_with_either_payload_shape(
    #[case] local_tags_as_maps: bool,
    #[case] expected_payload: Value,
    #[values("!T 5", "%TAG !v! !\n--- !v!T 5")] yaml: &str,
) {
    use serde_saphyr::Tagged;

    let opts = options! { local_tags_as_maps: local_tags_as_maps };
    let value: Tagged<Value> = from_str_with_options(yaml, opts.clone()).unwrap();
    assert_eq!(value, Tagged(expected_payload, Some("!T".into())));

    let number: Tagged<u32> = from_str_with_options(yaml, opts).unwrap();
    assert_eq!(number, Tagged(5, Some("!T".into())));
}

#[test]
fn local_tags_as_maps_preserves_tags_inside_repeated_aliases() {
    let opts = options! { local_tags_as_maps: true };
    let value: Value = from_str_with_options("[&items [!Add 5], *items, *items]", opts).unwrap();
    let items = json!([{"Add": 5}]);
    assert_eq!(value, json!([items, items, items]));
}

#[test]
fn local_tags_as_maps_charges_one_level_of_depth_per_tag() {
    use serde_saphyr::budget::BudgetBreach;

    let yaml = "[!Outer [!Inner [1]]]";
    let parse = |local_tags_as_maps: bool, max_depth: usize| {
        let opts = options! {
            local_tags_as_maps: local_tags_as_maps,
            budget: serde_saphyr::budget! { max_depth: max_depth },
        };
        from_str_with_options::<Value>(yaml, opts)
    };

    assert_eq!(parse(false, 3).unwrap(), json!([[[1]]]));
    let err = parse(true, 4).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::Depth { depth: 5 },
            ..
        }
    ));
    assert_eq!(
        parse(true, 5).unwrap(),
        json!([{"Outer": [{"Inner": [1]}]}])
    );
}

#[cfg(feature = "robotics")]
#[test]
fn local_tags_as_maps_with_angle_conversions() {
    let opts = options! { local_tags_as_maps: true, angle_conversions: true };
    let value: Value = from_str_with_options("[!degrees 180, !!degrees 180]", opts).unwrap();
    assert_eq!(value, json!([{"degrees": 180}, std::f64::consts::PI]));
}

#[test]
fn local_tags_as_maps_respects_strict_mode() {
    let opts = options! { local_tags_as_maps: true, reject_unsupported_tags: true };
    let error = from_str_with_options::<Value>("!Add 5", opts).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[test]
fn untagged_enum_reads_a_local_extension_as_a_map() {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Reading {
        Map(BTreeMap<String, Vec<u8>>),
    }

    let yaml = "!time [1, 2]";
    assert!(from_str::<Reading>(yaml).is_err());
    let opts = options! { local_tags_as_maps: true };
    let Reading::Map(map) = from_str_with_options(yaml, opts).unwrap();
    assert_eq!(map["time"], [1, 2]);
}

#[rstest]
#[case("!Add 5", "Add")]
#[case("!123 5", "123")]
#[case("!null 5", "null")]
#[case("!%24%7BPORT%7D 5", "${PORT}")]
fn local_tag_map_keys_support_string_newtypes(#[case] tagged: &str, #[case] name: &str) {
    use serde::de::{Deserializer, MapAccess, Visitor};
    use std::fmt;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TagName(Option<String>);

    struct Entry;

    impl<'de> Visitor<'de> for Entry {
        type Value = Option<(TagName, u8)>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a single-entry map")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            map.next_entry()
        }
    }

    let opts = options! { local_tags_as_maps: true };
    #[cfg(feature = "properties")]
    let opts = opts.with_properties([("PORT".into(), "80".into())].into());
    let explicit = format!("{{'{name}': 5}}");
    for yaml in [explicit.as_str(), tagged] {
        let entry =
            serde_saphyr::with_deserializer_from_str_with_options(yaml, opts.clone(), |de| {
                de.deserialize_any(Entry)
            })
            .unwrap();
        assert_eq!(entry, Some((TagName(Some(name.to_owned())), 5)));
    }
}

#[cfg(feature = "properties")]
#[test]
fn local_tags_as_maps_leaves_properties_in_keys_alone() {
    #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(untagged)]
    enum Key {
        Map(BTreeMap<String, String>),
    }

    let opts = options! { local_tags_as_maps: true }
        .with_properties([("PORT".into(), "80".into())].into());
    let map: BTreeMap<Key, u8> = from_str_with_options("? !Tag ${PORT}\n: 1", opts).unwrap();
    let key = Key::Map(BTreeMap::from([("Tag".into(), "${PORT}".into())]));
    assert_eq!(map.get(&key), Some(&1));
}

#[cfg(feature = "comments")]
#[test]
fn local_tags_as_maps_keeps_payload_comments() {
    use serde::de::{Deserializer, MapAccess, Visitor};
    use serde_saphyr::{Commented, with_deserializer_from_str_with_options};
    use std::fmt;

    struct Entry;

    impl<'de> Visitor<'de> for Entry {
        type Value = Option<(String, Vec<Commented<u8>>)>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a single-entry map")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            map.next_entry()
        }
    }

    let yaml = "!Batch\n  - 1\n  # second item\n  - 2";
    let opts = options! { local_tags_as_maps: true };
    let (tag, items) =
        with_deserializer_from_str_with_options(yaml, opts, |de| de.deserialize_any(Entry))
            .unwrap()
            .unwrap();
    assert_eq!(tag, "Batch");
    assert_eq!(items[1], Commented(2, "second item".into()));
}

#[rstest]
fn strict_mode_rejects_local_type_names(
    #[values(
        "int",
        "float",
        "bool",
        "null",
        "seq",
        "map",
        "str",
        "timestamp",
        "binary"
    )]
    tag: &str,
    #[values("", "text", "[1, 2]", "{value: 1}")] payload: &str,
) {
    let yaml = format!("!{tag} {payload}");
    let opts = options! { reject_unsupported_tags: true };
    let ignored = from_str_with_options::<IgnoredAny>(&yaml, opts.clone()).unwrap_err();
    let typed = from_str_with_options::<Unit>(&yaml, opts).unwrap_err();
    for error in [ignored, typed] {
        assert!(matches!(
            error.without_snippet(),
            Error::UnsupportedTag { .. }
        ));
    }
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, vec![1, 2])]
#[case(DuplicateKeyPolicy::LastWins, vec![3, 4])]
fn duplicate_values_with_local_enum_tags_keep_the_selected_value(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Vec<u8>,
) {
    // Ordinary local tags do not impose a built-in node shape, even while buffering.
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
    let yaml = format!("!{tag} [1, 2]");
    assert_eq!(from_str::<Extension>(&yaml).unwrap(), expected);

    let opts = options! { local_tags_as_maps: true };
    let value: Value = from_str_with_options(&yaml, opts).unwrap();
    assert_eq!(
        serde_json::from_value::<Extension>(value).unwrap(),
        expected
    );
}

#[rstest]
#[case("!time [1, 2]")]
#[case("!degrees {value: 1}")]
#[case("!radians []")]
fn ordinary_consumers_validate_robotics_collection_shapes(#[case] yaml: &str) {
    let error = from_str::<serde_json::Value>(yaml).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::UnsupportedTag { .. }
    ));
}

#[rstest]
#[case(DuplicateKeyPolicy::FirstWins, vec![1, 2])]
#[case(DuplicateKeyPolicy::LastWins, vec![3, 4])]
fn discarded_robotics_selectors_defer_shape_checks(
    #[case] duplicate_keys: DuplicateKeyPolicy,
    #[case] expected: Vec<u8>,
) {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Extension {
        #[serde(rename = "time")]
        Time(Vec<u8>),
    }
    let value: BTreeMap<String, Extension> = from_str_with_options(
        "value: !time [1, 2]\nvalue: !time [3, 4]\n",
        options! { duplicate_keys: duplicate_keys },
    )
    .unwrap();
    assert_eq!(
        value,
        BTreeMap::from([("value".into(), Extension::Time(expected))])
    );
}
