use std::collections::BTreeMap;

// WZ62: Spec Example 7.2. Empty Content
// Flow mapping with explicit !!str tags resulting in {"foo": "", "": "bar"}

#[test]
fn yaml_wz62_empty_content_in_flow_mapping() {
    let y = r#"{
  foo : !!str,
  !!str : bar,
}
"#;

    // Use BTreeMap for deterministic iteration order if needed
    let m: BTreeMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse WZ62");

    assert_eq!(m.len(), 2);
    assert_eq!(m.get("foo").unwrap(), "");
    assert_eq!(m.get("").unwrap(), "bar");
}
