use serde::Deserialize;

// Z67P: Block scalar nodes with explicit indent indicators and a local tag on folded
// According to the suite JSON, both values should deserialize to "value\n".

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    literal: String,
    folded: String,
}

#[test]
fn yaml_z67p_block_scalar_nodes() {
    let y = r#"literal: |2
  value
folded: !foo >1
 value
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse Z67P");
    assert_eq!(v.literal, "value\n");
    assert_eq!(v.folded, "value\n");
}
