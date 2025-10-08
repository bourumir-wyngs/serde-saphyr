use std::collections::HashMap;

// BU8L: Node Anchor and Tag on Separate Lines — key maps to a tagged map {a: b}
#[test]
fn yaml_bu8l_node_anchor_and_tag_on_separate_lines() {
    // Follow the suite structure: value under 'key' is anchored, then tagged as !!map, then contains a: b
    let y = "key: &anchor\n !!map\n  a: b\n";
    // Deserialize into mapping: key -> nested map
    let m: HashMap<String, HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse BU8L");

    let inner = m.get("key").expect("missing 'key'");
    assert_eq!(inner.get("a").map(String::as_str), Some("b"));
}