use std::collections::HashMap;

// MXS3: Flow Mapping in Block Sequence
#[test]
fn yaml_mxs3_flow_mapping_in_block_sequence() {
    let y = r#"- {a: b}
"#;
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse MXS3");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].get("a").map(String::as_str), Some("b"));
}
