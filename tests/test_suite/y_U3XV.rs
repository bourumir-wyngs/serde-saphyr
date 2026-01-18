use serde::Deserialize;
use std::collections::BTreeMap;

// U3XV: Node and Mapping Key Anchors
// Anchors on nodes and keys should not affect the final data model.
// Expected JSON shows a plain mapping; we deserialize into a struct to assert values.

#[derive(Debug, Deserialize)]
struct Root {
    top1: BTreeMap<String, String>,
    top2: BTreeMap<String, String>,
    top3: BTreeMap<String, String>,
    top4: BTreeMap<String, String>,
    top5: BTreeMap<String, String>,
    top6: String,
    top7: String,
}

#[test]
fn yaml_u3xv_node_and_key_anchors() {
    let y = r#"---
    top1: &node1
      &k1 key1: one
    top2: &node2 # comment
      key2: two
    top3:
      &k3 key3: three
    top4:
      &node4
      &k4 key4: four
    top5:
      &node5
      key5: five
    top6: &val6
      six
    top7:
      &val7 seven
"#;

    let r: Root = serde_saphyr::from_str(y).expect("failed to parse U3XV");

    assert_eq!(r.top1.get("key1").map(String::as_str), Some("one"));
    assert_eq!(r.top2.get("key2").map(String::as_str), Some("two"));
    assert_eq!(r.top3.get("key3").map(String::as_str), Some("three"));
    assert_eq!(r.top4.get("key4").map(String::as_str), Some("four"));
    assert_eq!(r.top5.get("key5").map(String::as_str), Some("five"));
    assert_eq!(r.top6, "six");
    assert_eq!(r.top7, "seven");
}
