#![cfg(feature = "deserialize")]
use rstest::rstest;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_saphyr::{Commented, Spanned};
use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum Value {
    Expression(String),
    Template(String),
    Pair(String, u32),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Context {
    value: Value,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TaggedScalar {
    Relative(i32),
    Unsigned(u32),
    Boolean(bool),
    Text(String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TaggedMapping {
    Map(BTreeMap<String, u32>),
    Dictionary(BTreeMap<String, u32>),
    Struct { a: u32, b: u32 },
    Commented { value: Commented<u32> },
    Spanned { value: Spanned<u32> },
    Unit,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum UntaggedMapping {
    Map(BTreeMap<String, u32>),
    Text(String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum UntaggedTaggedMapping {
    Tagged(TaggedMapping),
    Text(String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct TaggedMappingContainer {
    value: TaggedMapping,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind")]
enum InternallyTaggedMapping {
    Struct { a: u32, b: u32 },
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "data")]
enum AdjacentlyTaggedMapping {
    Struct { a: u32, b: u32 },
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct FlattenedTaggedMappings {
    #[serde(flatten)]
    values: BTreeMap<String, TaggedMapping>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum TaggedKey {
    First { x: u32 },
    Second { x: u32 },
    Unit,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum TaggedScalarKey {
    First(u32),
    Second(u32),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum TaggedSequenceKey {
    First(Vec<u32>),
    Second(Vec<u32>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "camelCase")]
enum MapCompatibilityKey {
    Map(BTreeMap<String, u32>),
    X(u32),
}

const TAGGED_MAPPING_KEYS: &str = "? !First {x: 1}\n\
                                   : 10\n\
                                   ? !Second {x: 1}\n\
                                   : 20\n";

#[test]
fn local_tags_do_not_become_core_string_tags() {
    for (yaml, expected) in [
        ("!relative 5", TaggedScalar::Relative(5)),
        ("!relative -3", TaggedScalar::Relative(-3)),
        ("!relative 0", TaggedScalar::Relative(0)),
        ("!unsigned 7", TaggedScalar::Unsigned(7)),
        ("!boolean true", TaggedScalar::Boolean(true)),
        ("!text 5", TaggedScalar::Text("5".to_owned())),
    ] {
        let actual: TaggedScalar =
            serde_saphyr::from_str(yaml).expect("tagged scalar should deserialize");

        assert_eq!(actual, expected, "failed to deserialize {yaml:?}");
    }
}

/// Regression test for <https://github.com/bourumir-wyngs/serde-saphyr/issues/177>.
#[rstest]
#[case::flow(r#"!map {"value": 10}"#)]
#[case::block("!map\n  value: 10")]
fn local_tag_selects_map_newtype_variant(#[case] yaml: &str) {
    let actual: TaggedMapping =
        serde_saphyr::from_str(yaml).expect("tagged mapping should deserialize");

    assert_eq!(
        actual,
        TaggedMapping::Map(BTreeMap::from([("value".to_owned(), 10)])),
        "failed to deserialize {yaml:?}"
    );
}

#[test]
fn custom_local_tag_selects_map_newtype_variant() {
    let actual: TaggedMapping = serde_saphyr::from_str(r#"!dictionary {"value": 10}"#)
        .expect("tagged mapping should deserialize");

    assert_eq!(
        actual,
        TaggedMapping::Dictionary(BTreeMap::from([("value".to_owned(), 10)]))
    );
}

#[rstest]
#[case::core_shorthand(r#"!!map {map: {"value": 10}}"#)]
#[case::core_verbatim(r#"!<tag:yaml.org,2002:map> {map: {"value": 10}}"#)]
#[case::core_directive("%TAG !m! tag:yaml.org,2002:\n--- !m!map {map: {value: 10}}")]
fn core_map_tag_does_not_select_enum_variant(#[case] yaml: &str) {
    let actual: TaggedMapping =
        serde_saphyr::from_str(yaml).expect("core-tagged mapping should deserialize");

    assert_eq!(
        actual,
        TaggedMapping::Map(BTreeMap::from([("value".to_owned(), 10)]))
    );
}

#[test]
fn local_map_compatibility_tag_remains_a_map_in_untagged_enum() {
    let actual: UntaggedMapping = serde_saphyr::from_str(r#"!map {"value": 10}"#)
        .expect("compatibility-tagged mapping should deserialize");

    assert_eq!(
        actual,
        UntaggedMapping::Map(BTreeMap::from([("value".to_owned(), 10)]))
    );
}

#[test]
fn custom_mapping_tag_survives_untagged_enum_buffering() {
    let actual: UntaggedTaggedMapping = serde_saphyr::from_str("!struct\n  a: 123\n  b: 456")
        .expect("tagged mapping in an untagged enum should deserialize");

    assert_eq!(
        actual,
        UntaggedTaggedMapping::Tagged(TaggedMapping::Struct { a: 123, b: 456 })
    );
}

#[test]
fn custom_yaml_mapping_tag_does_not_hide_serde_internal_tag() {
    let actual: InternallyTaggedMapping =
        serde_saphyr::from_str("!metadata {kind: Struct, a: 1, b: 2}")
            .expect("the YAML tag must not rewrite an internally tagged enum");

    assert_eq!(actual, InternallyTaggedMapping::Struct { a: 1, b: 2 });
}

#[test]
fn custom_yaml_mapping_tag_does_not_hide_serde_adjacent_tag() {
    let actual: AdjacentlyTaggedMapping =
        serde_saphyr::from_str("!metadata {data: {a: 1, b: 2}, kind: Struct}")
            .expect("the YAML tag must not rewrite an adjacently tagged enum");

    assert_eq!(actual, AdjacentlyTaggedMapping::Struct { a: 1, b: 2 });
}

#[test]
fn custom_mapping_tag_survives_flatten_buffering() {
    let actual: FlattenedTaggedMappings = serde_saphyr::from_str("value: !struct {a: 1, b: 2}")
        .expect("a custom YAML tag buffered by flatten should retain its enum variant");

    assert_eq!(
        actual,
        FlattenedTaggedMappings {
            values: BTreeMap::from([("value".to_owned(), TaggedMapping::Struct { a: 1, b: 2 },)]),
        }
    );
}

#[test]
fn differently_tagged_mapping_keys_are_not_duplicates() {
    let actual: BTreeMap<TaggedKey, u32> = serde_saphyr::from_str(TAGGED_MAPPING_KEYS)
        .expect("different tagged enum keys must not be considered duplicates");

    assert_eq!(actual.len(), 2);
    assert_eq!(actual.get(&TaggedKey::First { x: 1 }), Some(&10));
    assert_eq!(actual.get(&TaggedKey::Second { x: 1 }), Some(&20));
}

#[test]
fn custom_tagged_empty_mapping_key_is_not_an_implicit_empty_key() {
    let actual: BTreeMap<TaggedKey, u32> = serde_saphyr::from_str("? !Unit {}\n: 10\n")
        .expect("a tagged empty mapping must remain an enum key payload");

    assert_eq!(actual, BTreeMap::from([(TaggedKey::Unit, 10)]));
}

#[test]
fn first_wins_keeps_differently_tagged_mapping_keys() {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::FirstWins,
    };
    let actual: BTreeMap<TaggedKey, u32> =
        serde_saphyr::from_str_with_options(TAGGED_MAPPING_KEYS, options)
            .expect("first-wins must not discard a differently tagged key");

    assert_eq!(actual.len(), 2);
    assert_eq!(actual.get(&TaggedKey::First { x: 1 }), Some(&10));
    assert_eq!(actual.get(&TaggedKey::Second { x: 1 }), Some(&20));
}

#[test]
fn last_wins_merge_keeps_differently_tagged_mapping_keys() {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::LastWins,
    };
    let actual: BTreeMap<String, BTreeMap<TaggedKey, u32>> = serde_saphyr::from_str_with_options(
        concat!(
            "base: &base\n",
            "  ? !Second {x: 1}\n",
            "  : 20\n",
            "target:\n",
            "  ? !First {x: 1}\n",
            "  : 10\n",
            "  <<: *base\n",
        ),
        options,
    )
    .expect("last-wins merge filtering must preserve a differently tagged key");

    let target = &actual["target"];
    assert_eq!(target.len(), 2);
    assert_eq!(target.get(&TaggedKey::First { x: 1 }), Some(&10));
    assert_eq!(target.get(&TaggedKey::Second { x: 1 }), Some(&20));
}

#[test]
fn differently_tagged_scalar_keys_are_not_duplicates() {
    let actual: BTreeMap<TaggedScalarKey, u32> = serde_saphyr::from_str(
        "? !First 1\n\
         : 10\n\
         ? !Second 1\n\
         : 20\n",
    )
    .expect("different scalar tags must remain distinct key identities");

    assert_eq!(actual.len(), 2);
    assert_eq!(actual.get(&TaggedScalarKey::First(1)), Some(&10));
    assert_eq!(actual.get(&TaggedScalarKey::Second(1)), Some(&20));
}

#[test]
fn differently_tagged_sequence_keys_are_not_duplicates() {
    let actual: BTreeMap<TaggedSequenceKey, u32> = serde_saphyr::from_str(
        "? !First [1]\n\
         : 10\n\
         ? !Second [1]\n\
         : 20\n",
    )
    .expect("different sequence tags must remain distinct key identities");

    assert_eq!(actual.len(), 2);
    assert_eq!(actual.get(&TaggedSequenceKey::First(vec![1])), Some(&10));
    assert_eq!(actual.get(&TaggedSequenceKey::Second(vec![1])), Some(&20));
}

#[test]
fn local_map_tag_is_distinct_from_resolved_core_map_tag_in_keys() {
    let actual: BTreeMap<MapCompatibilityKey, u32> = serde_saphyr::from_str(
        "? !map {x: 1}\n\
         : 10\n\
         ? !!map {x: 1}\n\
         : 20\n",
    )
    .expect("local !map and YAML core map must have distinct key identities");

    assert_eq!(actual.len(), 2);
    assert_eq!(
        actual.get(&MapCompatibilityKey::Map(BTreeMap::from([(
            "x".to_owned(),
            1,
        )]))),
        Some(&10)
    );
    assert_eq!(actual.get(&MapCompatibilityKey::X(1)), Some(&20));
}

#[test]
fn equivalent_core_map_tag_spellings_remain_duplicate_keys() {
    let error = serde_saphyr::from_str::<BTreeMap<MapCompatibilityKey, u32>>(
        "? !!map {x: 1}\n\
         : 10\n\
         ? !<tag:yaml.org,2002:map> {x: 1}\n\
         : 20\n",
    )
    .expect_err("equivalent resolved core tags must have the same key identity");
    let error = error.without_snippet();

    assert!(matches!(
        error,
        serde_saphyr::Error::DuplicateMappingKey { key: None, .. }
    ));
}

#[test]
fn equivalent_resolved_custom_tag_spellings_remain_duplicate_keys() {
    let error = serde_saphyr::from_str::<BTreeMap<TaggedKey, u32>>(
        "%TAG !f! !\n\
         ---\n\
         ? !First {x: 1}\n\
         : 10\n\
         ? !f!First {x: 1}\n\
         : 20\n",
    )
    .expect_err("equivalent resolved custom tags must have the same key identity");
    let error = error.without_snippet();

    assert!(matches!(
        error,
        serde_saphyr::Error::DuplicateMappingKey { key: None, .. }
    ));
}

#[test]
fn tagged_mapping_payload_preserves_inner_comments() {
    let actual: TaggedMapping = serde_saphyr::from_str("!commented\n  # note\n  value: 10")
        .expect("tagged mapping with a comment should deserialize");

    assert_eq!(
        actual,
        TaggedMapping::Commented {
            value: Commented(10, "note".to_owned())
        }
    );
}

#[test]
fn tagged_mapping_variant_deserializes_from_a_buffered_map_value() {
    let actual: TaggedMappingContainer =
        serde_saphyr::from_str("value: !struct\n  a: 123\n  b: 456")
            .expect("nested tagged mapping should deserialize");

    assert_eq!(
        actual,
        TaggedMappingContainer {
            value: TaggedMapping::Struct { a: 123, b: 456 }
        }
    );
}

#[test]
fn tagged_mapping_alias_preserves_nested_use_site_location() {
    let actual: Vec<TaggedMapping> =
        serde_saphyr::from_str("- &tagged !spanned\n  value: 10\n- *tagged\n")
            .expect("tagged mapping alias should deserialize");

    let TaggedMapping::Spanned { value } = &actual[1] else {
        panic!("expected spanned variant");
    };
    assert_eq!(value.value, 10);
    assert_eq!(value.referenced.line(), 3);
    assert_eq!(value.defined.line(), 2);
}

#[test]
fn tagged_unit_mapping_payload_is_drained_before_the_next_item() {
    let actual: Vec<TaggedMapping> =
        serde_saphyr::from_str("- !unit {ignored: [1, 2]}\n- !struct\n  a: 123\n  b: 456\n")
            .expect("tagged mapping sequence should deserialize");

    assert_eq!(
        actual,
        vec![
            TaggedMapping::Unit,
            TaggedMapping::Struct { a: 123, b: 456 }
        ]
    );
}

/// Regression test for <https://github.com/bourumir-wyngs/serde-saphyr/issues/177>.
#[rstest]
#[case::flow(r#"!struct {"a": 123, "b": 456}"#)]
#[case::block("!struct\n  a: 123\n  b: 456")]
fn local_tag_selects_struct_variant(#[case] yaml: &str) {
    let actual: TaggedMapping =
        serde_saphyr::from_str(yaml).expect("tagged mapping should deserialize");

    assert_eq!(actual, TaggedMapping::Struct { a: 123, b: 456 });
}

#[test]
fn test_tagged_expression_scalar() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Expression "1 + 1""#),
        Context {
            value: Value::Expression("1 + 1".to_string())
        }
    );
}

#[test]
fn test_tagged_pair_flow_seq() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Pair [a, 12]"#),
        Context {
            value: Value::Pair("a".to_string(), 12)
        }
    );
}

#[test]
fn test_tagged_pair_block_seq() {
    assert_eq!(
        serde_saphyr::<Context>(
            r#"
value: !Pair
  - "a"
  - 12
"#
        ),
        Context {
            value: Value::Pair("a".to_string(), 12)
        }
    );
}

#[test]
fn test_tagged_pair_wrong_shape_scalar_should_error() {
    // arity>1 should *not* accept scalar
    let err = serde_saphyr::from_str::<Context>(r#"value: !Pair "a""#).unwrap_err();
    let err = err.without_snippet();
    assert!(matches!(
        err,
        serde_saphyr::Error::Unexpected {
            expected: "sequence start",
            ..
        }
    ));
}

#[track_caller]
fn serde_saphyr<T: DeserializeOwned + Debug>(yaml: &str) -> T {
    match serde_saphyr::from_str::<T>(yaml) {
        Ok(value) => value,
        Err(err) => panic!("{}", err),
    }
}
