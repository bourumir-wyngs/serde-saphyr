// ZCZ6: Invalid mapping in plain single line value
// YAML: a: b: c: d
// The test-suite marks this as fail: true. Our parser should report an error.

#[test]
fn yaml_zcz6_invalid_mapping_in_plain_single_line_value() {
    let y = r#"a: b: c: d"#;
    let result = serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y);
    assert!(
        result.is_err(),
        "ZCZ6 should be invalid YAML (mapping in plain scalar) but parser accepted it: {:?}",
        result
    );
}
