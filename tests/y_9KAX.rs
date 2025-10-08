use serde::Deserialize;
use std::collections::HashMap;

// 9KAX: Various combinations of tags and anchors — multi-document stream
// Expect documents: "scalar1", "scalar2", "scalar3", {key5: value4}, {a6:1, b6:2}, {key8: value7}, {key10: value9}, "value11".
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Doc {
    S(String),
    MStr(HashMap<String, String>),
    MInt(HashMap<String, i32>),
}

#[test]
fn yaml_9kax_various_combinations_tags_anchors() {
    let y = r#"---
&a1
!!str
scalar1
---
!!str
&a2
scalar2
---
&a3
!!str scalar3
---
&a4 !!map
&a5 !!str key5: value4
---
a6: 1
&anchor6 b6: 2
---
!!map
&a8 !!str key8: value7
---
!!map
!!str &a10 key10: value9
---
!!str &a11
value11
"#;
    let docs: Vec<Doc> = serde_saphyr::from_multiple(y).expect("failed to parse 9KAX");
    assert_eq!(docs.len(), 8);

    assert!(matches!(&docs[0], Doc::S(s) if s == "scalar1"));
    assert!(matches!(&docs[1], Doc::S(s) if s == "scalar2"));
    assert!(matches!(&docs[2], Doc::S(s) if s == "scalar3"));

    match &docs[3] {
        Doc::MStr(m) => assert_eq!(m.get("key5").map(String::as_str), Some("value4")),
        other => panic!("doc3 expected MStr, got: {:?}", other),
    }
    match &docs[4] {
        Doc::MInt(m) => {
            assert_eq!(m.get("a6"), Some(&1));
            assert_eq!(m.get("b6"), Some(&2));
        }
        Doc::MStr(m) => {
            assert_eq!(m.get("a6").map(String::as_str), Some("1"));
            assert_eq!(m.get("b6").map(String::as_str), Some("2"));
        }
        other => panic!("doc4 expected MInt or MStr, got: {:?}", other),
    }
    match &docs[5] {
        Doc::MStr(m) => assert_eq!(m.get("key8").map(String::as_str), Some("value7")),
        other => panic!("doc5 expected MStr, got: {:?}", other),
    }
    match &docs[6] {
        Doc::MStr(m) => assert_eq!(m.get("key10").map(String::as_str), Some("value9")),
        other => panic!("doc6 expected MStr, got: {:?}", other),
    }
    assert!(matches!(&docs[7], Doc::S(s) if s == "value11"));
}