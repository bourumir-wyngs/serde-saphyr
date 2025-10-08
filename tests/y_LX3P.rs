use std::collections::HashMap;

// LX3P: Implicit Flow Mapping Key on one line
// Key is a flow sequence [flow], value is the scalar "block".
#[test]
fn yaml_lx3p_implicit_flow_mapping_key_on_one_line() {
    let y = r#"[flow]: block
"#;
    let v: HashMap<Vec<String>, String> = serde_saphyr::from_str(y).expect("failed to parse LX3P");
    assert_eq!(v.len(), 1);
    assert_eq!(v.get(&vec!["flow".to_string()]).map(String::as_str), Some("block"));
}
