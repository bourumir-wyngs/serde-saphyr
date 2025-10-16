use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct SuiteCase {
    yaml: String,
}

#[derive(Debug, Deserialize)]
struct Root {
    top1: Vec<Top1Item>,
    top2: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Top1Item {
    Str(String),
    Map(BTreeMap<String, String>),
}

#[test]
fn y_R52L() {
    let yaml = r#"---
{ top1: [item1, {key2: value2}, item3], top2: value2 }
"#;

    let v: Root = serde_saphyr::from_str(yaml).expect("parse inner YAML");

    assert_eq!(v.top2, "value2");
    assert_eq!(v.top1.len(), 3);

    match &v.top1[0] {
        Top1Item::Str(s) => assert_eq!(s, "item1"),
        _ => panic!("expected first element to be string 'item1'"),
    }

    match &v.top1[1] {
        Top1Item::Map(m) => {
            assert_eq!(m.len(), 1);
            assert_eq!(m.get("key2").map(|s| s.as_str()), Some("value2"));
        }
        _ => panic!("expected second element to be a map with key2: value2"),
    }

    match &v.top1[2] {
        Top1Item::Str(s) => assert_eq!(s, "item3"),
        _ => panic!("expected third element to be string 'item3'"),
    }
}
