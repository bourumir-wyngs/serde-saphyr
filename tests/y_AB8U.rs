// AB8U: Sequence entry that looks like two with wrong indentation — should parse as one folded scalar
#[test]
fn yaml_ab8u_sequence_entry_looks_like_two() {
    let y = "- single multiline\n - sequence entry\n";
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse AB8U");
    assert_eq!(v, vec!["single multiline - sequence entry".to_string()]);
}
