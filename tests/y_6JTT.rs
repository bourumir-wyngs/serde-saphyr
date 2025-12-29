// 6JTT: Flow sequence without closing bracket — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_6jtt_flow_sequence_without_closing_bracket_should_fail() {
    let y = "---\n[ [ a, b, c ]\n";
    let result: Result<Vec<Vec<String>>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "6JTT should fail to parse due to missing closing bracket"
    );
}
