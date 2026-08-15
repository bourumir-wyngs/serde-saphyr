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
