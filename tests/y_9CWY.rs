// 9CWY: Invalid scalar at the end of mapping — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_9cwy_invalid_scalar_at_end_of_mapping_should_fail() {
    let y = "key:\n - item1\n - item2\ninvalid\n";
    let result: Result<std::collections::HashMap<String, Vec<String>>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "9CWY should fail to parse due to stray scalar after mapping");
}