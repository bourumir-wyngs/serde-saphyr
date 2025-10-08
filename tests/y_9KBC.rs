// 9KBC: Mapping starting at --- line — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_9kbc_mapping_starting_at_doc_marker_should_fail() {
    let y = "--- key1: value1\n    key2: value2\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "9KBC should fail due to invalid mapping starting at --- line");
}