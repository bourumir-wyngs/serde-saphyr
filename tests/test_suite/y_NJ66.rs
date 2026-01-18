use std::collections::HashMap;

// NJ66: Multiline plain flow mapping key
#[test]
fn yaml_nj66_multiline_plain_flow_mapping_key() {
    let y = r#"---
- { single line: value}
- { multi
  line: value}
"#;
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse NJ66");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].get("single line").map(String::as_str), Some("value"));
    assert_eq!(v[1].get("multi line").map(String::as_str), Some("value"));
}
