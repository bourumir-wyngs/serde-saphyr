// 236B: Invalid value after mapping — marked fail: true in the suite
// Expect parsing to return an error (no panic).
#[test]
fn yaml_236b_invalid_value_after_mapping_should_fail() {
    let y = "foo:\n  bar\ninvalid\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "236B should fail due to stray scalar after mapping"
    );
}
