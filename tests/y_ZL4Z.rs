// ZL4Z: Invalid nested mapping
// YAML:
// ---\n// a: 'b': c
// The suite marks this as fail: true. Our parser should reject it.

#[test]
fn yaml_zl4z_invalid_nested_mapping() {
    let y = r#"---
 a: 'b': c
"#;
    let result = serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y);
    assert!(
        result.is_err(),
        "ZL4Z should be invalid YAML (nested mapping in plain/single-quoted) but parser accepted it: {:?}",
        result
    );
}
