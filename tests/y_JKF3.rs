use serde::Deserialize;

// JKF3: Multiline unindented double-quoted block key — marked fail: true in suite
// Expect parsing to fail (no panic).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy(Vec<Vec<std::collections::HashMap<String, String>>>);

#[test]
fn yaml_jkf3_multiline_unindented_double_quoted_block_key_should_fail() {
    let y = r#"- - "bar
bar": x
"#;
    let result: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "JKF3 should fail to parse due to unindented continuation of quoted key"
    );
}
