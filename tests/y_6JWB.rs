use serde::Deserialize;
use std::collections::HashMap;

// 6JWB: Tags for Block Objects — mapping foo -> sequence ["a", {key: "value"}]
#[derive(Debug, Deserialize)]
struct Doc {
    foo: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    S(String),
    M(HashMap<String, String>),
}

#[test]
fn yaml_6jwb_tags_for_block_objects() {
    let y = "foo: !!seq\n  - !!str a\n  - !!map\n    key: !!str value\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse 6JWB");
    assert_eq!(d.foo.len(), 2);
    match &d.foo[0] {
        Entry::S(s) => assert_eq!(s, "a"),
        _ => panic!("expected first entry to be string"),
    }
    match &d.foo[1] {
        Entry::M(m) => assert_eq!(m.get("key").map(String::as_str), Some("value")),
        _ => panic!("expected second entry to be mapping"),
    }
}
