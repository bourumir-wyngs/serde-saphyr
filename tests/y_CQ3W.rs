// CQ3W: Double quoted string without closing quote — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_cq3w_double_quoted_without_closing_should_fail() {
    let y = "---\nkey: \"missing closing quote\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "CQ3W should fail to parse due to missing closing quote");
}
