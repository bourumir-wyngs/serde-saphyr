// 9MQT: Scalar doc with '...' in content; second variant is fail: true
#[test]
fn yaml_9mqt_scalar_doc_with_ellipsis_in_content() {
    let y = "--- \"a\n...x\nb\"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 9MQT part 1");
    assert_eq!(s, "a ...x b");
}

#[test]
fn yaml_9mqt_scalar_doc_with_separated_ellipsis_should_fail() {
    let y = "--- \"a\n... x\nb\"\n";
    let result: Result<String, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "9MQT part 2 should fail to parse");
}
