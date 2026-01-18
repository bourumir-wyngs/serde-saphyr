// 7MNF: Missing colon — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_7mnf_missing_colon_should_fail() {
    let y = "top1:\n  key1: val1\ntop2\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "7MNF should fail to parse due to missing colon after 'top2'"
    );
}
