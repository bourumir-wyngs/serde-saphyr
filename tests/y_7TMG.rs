// 7TMG: Comment in flow sequence before comma
#[test]
fn yaml_7tmg_comment_in_flow_sequence_before_comma() {
    let y = "---\n[ word1\n# comment\n, word2]\n";
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse 7TMG");
    assert_eq!(v, vec!["word1".to_string(), "word2".to_string()]);
}
