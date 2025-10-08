// 5TRB: Invalid document-start marker in doublequoted string — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_5trb_invalid_doc_start_in_double_quoted_should_fail() {
    let y = "---\n\"\n---\n\"\n";
    let result: Result<String, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "5TRB should fail to parse due to invalid content");
}
