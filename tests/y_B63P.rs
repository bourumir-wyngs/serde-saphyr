// B63P: Directive without document — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_b63p_directive_without_document_should_fail() {
    let y = "%YAML 1.2\n...\n";
    let result: Result<String, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "B63P should fail because a directive without a following document is invalid");
}