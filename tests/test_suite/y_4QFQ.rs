// 4QFQ: Block Indentation Indicator [1.3]
#[test]
fn yaml_4qfq_block_indentation_indicator() {
    // Expect a sequence of four scalars with specific contents
    let y = "- |\n detected\n- >\n\n\n  # detected\n- |1\n  explicit\n- >\n detected\n";
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse 4QFQ");
    assert_eq!(v.len(), 4);
    assert_eq!(v[0], "detected\n");
    assert_eq!(v[1], "\n\n# detected\n");
    assert_eq!(v[2], " explicit\n");
    assert_eq!(v[3], "detected\n");
}
