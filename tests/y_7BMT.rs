use serde::Deserialize;
use std::collections::HashMap;

// 7BMT: Node and Mapping Key Anchors — mapping with various anchored keys/values
#[derive(Debug, Deserialize)]
struct Root {
    top1: HashMap<String, String>,
    top2: HashMap<String, String>,
    top3: HashMap<String, String>,
    top4: HashMap<String, String>,
    top5: HashMap<String, String>,
    top6: String,
    top7: String,
}

#[test]
fn yaml_7bmt_node_and_mapping_key_anchors() {
    let y = "---\ntop1: &node1\n  &k1 key1: one\ntop2: &node2 # comment\n  key2: two\ntop3:\n  &k3 key3: three\ntop4: &node4\n  &k4 key4: four\ntop5: &node5\n  key5: five\ntop6: &val6\n  six\ntop7:\n  &val7 seven\n";
    let r: Root = serde_saphyr::from_str(y).expect("failed to parse 7BMT");

    assert_eq!(r.top1.get("key1").map(String::as_str), Some("one"));
    assert_eq!(r.top2.get("key2").map(String::as_str), Some("two"));
    assert_eq!(r.top3.get("key3").map(String::as_str), Some("three"));
    assert_eq!(r.top4.get("key4").map(String::as_str), Some("four"));
    assert_eq!(r.top5.get("key5").map(String::as_str), Some("five"));
    assert_eq!(r.top6, "six");
    assert_eq!(r.top7, "seven");
}