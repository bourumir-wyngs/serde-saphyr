#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use serde_saphyr::{LitStr, to_string};
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename = "__yaml_anchor")]
struct AnchorPayloadWithExtra(usize, &'static str, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_weak_anchor")]
struct WeakAnchorPayloadWithExtra(usize, bool, &'static str, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_commented")]
struct CommentedPayloadWithExtra(&'static str, &'static str, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_anchor")]
struct AnchorPayloadWithBytes<'a>(&'a serde_bytes::Bytes, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_weak_anchor")]
struct WeakAnchorPayloadWithBytes(usize, serde_bytes::ByteBuf, &'static str);

#[derive(Serialize)]
#[serde(rename = "__yaml_commented")]
struct CommentedPayloadWithBytes(serde_bytes::ByteBuf, &'static str);

struct UnknownLenMap;

impl Serialize for UnknownLenMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("inner", &1)?;
        map.end()
    }
}

#[derive(Serialize)]
struct WrapUnknownLenMap {
    outer: UnknownLenMap,
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct FailingKey;

impl Serialize for FailingKey {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom("synthetic key failure"))
    }
}

#[derive(Serialize)]
struct BlockSiblingCollections {
    first: Vec<i32>,
    second: Vec<i32>,
    third: BTreeMap<&'static str, i32>,
}

fn assert_unexpected_error<T: Serialize>(value: &T, expected: &str) {
    let err = to_string(value).expect_err("serialization should fail");
    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Unexpected { msg } if msg == expected
    ));
}

fn assert_message_error<T: Serialize>(value: &T, expected: &str) {
    let err = to_string(value).expect_err("serialization should fail");
    assert!(matches!(
        err,
        serde_saphyr::ser_error::Error::Message { msg } if msg == expected
    ));
}

#[test]
fn internal_tuple_payloads_reject_extra_fields() {
    assert_unexpected_error(
        &AnchorPayloadWithExtra(1, "value", "extra"),
        "unexpected field in __yaml_anchor",
    );
    assert_unexpected_error(
        &WeakAnchorPayloadWithExtra(1, true, "value", "extra"),
        "unexpected field in __yaml_weak_anchor",
    );
    assert_unexpected_error(
        &CommentedPayloadWithExtra("comment", "value", "extra"),
        "unexpected field in __yaml_commented",
    );
}

#[test]
fn internal_tuple_payload_captures_reject_bytes_in_scalar_slots() {
    assert_unexpected_error(
        &AnchorPayloadWithBytes(serde_bytes::Bytes::new(b"ptr"), "value"),
        "ptr expects number",
    );
    assert_unexpected_error(
        &WeakAnchorPayloadWithBytes(1, serde_bytes::ByteBuf::from(b"present".to_vec()), "value"),
        "bool expected",
    );
    assert_unexpected_error(
        &CommentedPayloadWithBytes(serde_bytes::ByteBuf::from(b"comment".to_vec()), "value"),
        "str expected",
    );
}

#[test]
fn unknown_length_map_value_breaks_before_first_entry() {
    let yaml = to_string(&WrapUnknownLenMap {
        outer: UnknownLenMap,
    })
    .unwrap();

    assert_eq!(yaml, "outer:\n  inner: 1\n");
}

#[test]
fn complex_key_inside_sequence_aligns_value_under_dash() {
    let mut map = BTreeMap::new();
    map.insert(vec![1, 2], "value");

    let yaml = to_string(&vec![map]).unwrap();

    assert!(yaml.contains("? - 1"), "missing complex key marker: {yaml}");
    assert!(
        yaml.contains(": value"),
        "missing complex key value: {yaml}"
    );
}

#[test]
fn scalar_key_errors_are_propagated() {
    let mut map = BTreeMap::new();
    map.insert(FailingKey, 1);

    assert_message_error(&map, "synthetic key failure");
}

#[test]
fn block_collection_siblings_force_newline_for_following_values() {
    let mut third = BTreeMap::new();
    third.insert("answer", 42);
    let value = BlockSiblingCollections {
        first: vec![1],
        second: vec![2],
        third,
    };

    let yaml = to_string(&value).unwrap();

    assert_eq!(yaml, "first:\n- 1\nsecond:\n- 2\nthird:\n  answer: 42\n");
}

#[test]
fn explicit_literal_string_with_unsafe_content_falls_back_to_quotes() {
    let yaml = to_string(&LitStr("line\rbreak")).unwrap();

    assert_eq!(yaml, "\"line\\rbreak\"\n");
}
