// CML9: Missing comma in flow — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_cml9_missing_comma_in_flow_should_fail() {
    let y = "key: [ word1\n#  xxx\n  word2 ]\n";
    let result: Result<std::collections::HashMap<String, Vec<String>>, _> =
        serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "CML9 should fail to parse due to missing comma in flow sequence"
    );
}
