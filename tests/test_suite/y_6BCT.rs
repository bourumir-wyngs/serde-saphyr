use serde::Deserialize;
use std::collections::HashMap;

// 6BCT: Separation Spaces — sequence of a mapping and a nested sequence
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Item {
    Map(HashMap<String, String>),
    Seq(Vec<String>),
}

#[test]
fn yaml_6bct_separation_spaces() {
    let y = "- foo: bar\n- - baz\n  - baz\n";
    let v: Vec<Item> = serde_saphyr::from_str(y).expect("failed to parse 6BCT");
    assert_eq!(v.len(), 2);
    match &v[0] {
        Item::Map(m) => assert_eq!(m.get("foo").map(String::as_str), Some("bar")),
        _ => panic!("expected first element to be a mapping"),
    }
    match &v[1] {
        Item::Seq(s) => assert_eq!(s, &vec!["baz".to_string(), "baz".to_string()]),
        _ => panic!("expected second element to be a sequence"),
    }
}
