use std::collections::HashMap;

// 9SA2: Multiline double quoted flow mapping key
#[test]
fn yaml_9sa2_multiline_double_quoted_flow_mapping_key() {
    let y = "---\n- { \"single line\": value}\n- { \"multi\n  line\": value}\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse 9SA2");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].get("single line").map(String::as_str), Some("value"));
    assert_eq!(v[1].get("multi line").map(String::as_str), Some("value"));
}
