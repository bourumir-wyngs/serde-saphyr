// SKE5: Anchor before zero indented sequence
// Expected to parse into a mapping with key "seq" -> ["a","b"].

#[test]
fn y_ske5_parse_anchor_before_zero_indented_sequence() {
    let y = "---\nseq:\n &anchor\n- a\n- b\n";
    #[derive(Debug, serde::Deserialize)]
    struct Root {
        seq: Vec<String>,
    }
    let r: Result<Root, _> = serde_saphyr::from_str(y);
    assert!(
        r.is_ok(),
        "Parser failed to handle anchor-before-seq layout: {:?}",
        r
    );
    let root = r.unwrap();
    assert_eq!(root.seq, vec!["a".to_string(), "b".to_string()]);
}
