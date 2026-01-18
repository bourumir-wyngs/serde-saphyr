// 4EJS: Invalid tabs as indentation in a mapping — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_4ejs_invalid_tabs_in_mapping_should_fail() {
    let y = "---\na:\n\t b:\n\t\t c: value\n"; // Use real tabs to model the intent
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "4EJS should fail to parse due to tab indentation"
    );
}
