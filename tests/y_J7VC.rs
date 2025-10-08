use std::collections::HashMap;

// J7VC: Empty Lines Between Mapping Elements
#[test]
fn yaml_j7vc_empty_lines_between_mapping_elements() {
    let y = r#"one: 2


three: 4
"#;
    let v: HashMap<String, i32> = serde_saphyr::from_str(y).expect("failed to parse J7VC");
    assert_eq!(v.len(), 2);
    assert_eq!(v.get("one"), Some(&2));
    assert_eq!(v.get("three"), Some(&4));
}
