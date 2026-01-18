// 7LBH: Multiline double quoted implicit keys — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_7lbh_multiline_double_quoted_implicit_keys_should_fail() {
    let y = "\"a\nb\": 1\n\"c\n d\": 1\n";
    let result: Result<std::collections::HashMap<String, i32>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "7LBH should fail to parse due to multiline double-quoted implicit keys"
    );
}
