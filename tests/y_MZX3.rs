// MZX3: Non-Specific Tags on Scalars — sequence of various scalar forms
#[test]
fn yaml_mzx3_non_specific_tags_on_scalars() {
    let y = r#"-
  plain
- "double quoted"
- 'single quoted'
- >
  block
- plain again
"#;
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse MZX3");
    assert_eq!(v, vec![
        "plain".to_string(),
        "double quoted".to_string(),
        "single quoted".to_string(),
        "block\n".to_string(),
        "plain again".to_string(),
    ]);
}
