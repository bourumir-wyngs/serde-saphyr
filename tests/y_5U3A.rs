// 5U3A: Sequence on same Line as Mapping Key — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_5u3a_sequence_on_same_line_as_key_should_fail() {
    let y = "key: - a\n     - b\n";
    let result: Result<std::collections::HashMap<String, Vec<String>>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "5U3A should fail to parse due to sequence starting on same line as key");
}
