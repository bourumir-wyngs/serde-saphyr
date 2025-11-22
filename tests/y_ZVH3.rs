// ZVH3: Wrong indented sequence item
// YAML:
// - key: value
//  - item1
// The suite marks this as fail: true. Our parser should reject it.

#[test]
fn yaml_zvh3_wrong_indented_sequence_item() {
    let y = r#"- key: value
 - item1
"#;
    let result = serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y);
    assert!(
        result.is_err(),
        "ZVH3 should be invalid YAML (wrong indentation for sequence item) but parser accepted it: {:?}",
        result
    );
}
