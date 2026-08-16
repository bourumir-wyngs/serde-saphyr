#![cfg(feature = "deserialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/396.
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, PartialEq)]
enum TaggedNode {
    #[serde(rename = "tag")]
    Empty,
}

#[test]
fn tagged_empty_node_can_end_a_flow_sequence() {
    let actual: serde_json::Value =
        serde_saphyr::from_str("[!tag]").expect("tagged empty node should deserialize");
    assert_eq!(actual, json!([null]));

    let typed: Vec<TaggedNode> =
        serde_saphyr::from_str("[!tag]").expect("tag should select the unit variant");
    assert_eq!(typed, vec![TaggedNode::Empty]);
}
