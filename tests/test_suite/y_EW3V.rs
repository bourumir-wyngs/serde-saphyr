use serde::Deserialize;

// EW3V: Wrong indentation in mapping (invalid YAML). The snippet is expected to fail.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy {
    k1: String,
}

#[test]
fn yaml_ew3v_wrong_indentation_should_fail() {
    let y = r#"k1: v1
 k2: v2
"#;
    let result: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "EW3V should fail to parse due to wrong indentation"
    );
}
