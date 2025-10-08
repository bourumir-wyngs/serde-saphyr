// LE5A: Spec Example 7.24. Flow Nodes
// Sequence with tags, anchor and alias, and an explicit !!str with empty content.

#[test]
fn yaml_le5a_flow_nodes_with_tags_and_alias() {
    let y = r#"- !!str "a"
- 'b'
- &anchor "c"
- *anchor
- !!str
"#;
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse LE5A");
    assert_eq!(v, vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "c".to_string(),
        "".to_string(),
    ]);
}
