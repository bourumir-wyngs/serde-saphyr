use serde::Deserialize;

// ZK9H: Nested top level flow mapping with multi-line nested sequences
// { key: [[[\n  value\n ]]]\n}

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    key: Vec<Vec<Vec<String>>>,
}

#[test]
fn yaml_zk9h_nested_top_level_flow_mapping() {
    let y = r#"{ key: [[[
  value
 ]]]
}
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse ZK9H");
    assert_eq!(v.key.len(), 1);
    assert_eq!(v.key[0].len(), 1);
    assert_eq!(v.key[0][0], vec![String::from("value")]);
}
