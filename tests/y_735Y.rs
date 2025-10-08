use serde::Deserialize;
use std::collections::HashMap;

// 735Y: Block Node Types — sequence: string, folded scalar, and an explicitly tagged map
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Item {
    S(String),
    M(HashMap<String, String>),
}

#[test]
fn yaml_735y_block_node_types() {
    let y = "- \"flow in block\"\n- >\n Block scalar\n- !!map\n  foo : bar\n";
    let v: Vec<Item> = serde_saphyr::from_str(y).expect("failed to parse 735Y");
    assert_eq!(v.len(), 3);
    match &v[0] { Item::S(s) => assert_eq!(s, "flow in block"), _ => panic!("expected string") }
    match &v[1] { Item::S(s) => assert_eq!(s, "Block scalar\n"), _ => panic!("expected folded scalar as string") }
    match &v[2] {
        Item::M(m) => assert_eq!(m.get("foo").map(String::as_str), Some("bar")),
        _ => panic!("expected mapping"),
    }
}
