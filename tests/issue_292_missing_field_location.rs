#![cfg(feature = "deserialize")]

//! A missing field is reported at the start of the mapping it is missing from, as
//! serde_yaml does: the first key of a block mapping, or the `{` of a flow mapping.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Item {
    name: String,
    platform: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Root {
    item: Item,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Outer {
    item: Item,
    required: u32,
}

#[track_caller]
fn missing_field_at<T: DeserializeOwned + std::fmt::Debug>(yaml: &str) -> (u64, u64) {
    let err = serde_saphyr::from_str::<T>(yaml).expect_err("a field is missing");
    assert!(
        matches!(err.without_snippet(), Error::SerdeMissingField { .. }),
        "{yaml:?}: {err}"
    );
    let location = err.location().expect("a missing field has a location");
    (location.line(), location.column())
}

#[test]
fn top_level_block_mapping() {
    assert_eq!(
        missing_field_at::<Item>("name: test\nother: 1\nlast: 2\n"),
        (1, 1)
    );
}

#[test]
fn nested_block_mapping() {
    assert_eq!(
        missing_field_at::<Root>("item:\n  name: test\n  other: 1\n  last: 2\n"),
        (2, 3)
    );
}

#[test]
fn nested_flow_mapping() {
    assert_eq!(
        missing_field_at::<Root>("item: {name: test, other: 1}\n"),
        (1, 7)
    );
}

#[test]
fn mapping_in_a_sequence() {
    assert_eq!(
        missing_field_at::<Vec<Item>>("- name: a\n  platform: x\n- name: b\n  other: 1\n"),
        (3, 3)
    );
}

#[test]
fn outer_mapping_after_a_complete_inner_one() {
    assert_eq!(
        missing_field_at::<Outer>("item:\n  name: a\n  platform: b\nlast: 1\n"),
        (1, 1)
    );
}
