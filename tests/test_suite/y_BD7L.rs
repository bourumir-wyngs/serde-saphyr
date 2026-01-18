// BD7L: Invalid mapping after sequence — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_bd7l_invalid_mapping_after_sequence_should_fail() {
    let y = "- item1\n- item2\ninvalid: x\n";
    let result: Result<Vec<String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "BD7L should fail due to mapping entry after a top-level sequence"
    );
}
