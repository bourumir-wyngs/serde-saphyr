// ZXT5: Implicit key followed by newline and adjacent value (invalid)
// YAML:
// [ "key"\n//   :value ]
// The suite marks this as fail: true. Our parser should reject it.

#[test]
fn yaml_zxt5_implicit_key_newline_adjacent_value_invalid() {
    let y = r#"[ "key"
  :value ]
"#;
    let result = serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y);
    assert!(
        result.is_err(),
        "ZXT5 should be invalid YAML (implicit key followed by newline and adjacent value) but parser accepted it: {:?}",
        result
    );
}
