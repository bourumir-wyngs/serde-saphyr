use std::collections::HashMap;

// 5C5M: Flow Mappings in a sequence
#[test]
fn yaml_5c5m_flow_mappings() {
    let y = "- { one : two , three: four , }\n- {five: six,seven : eight}\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse 5C5M");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].get("one").map(String::as_str), Some("two"));
    assert_eq!(v[0].get("three").map(String::as_str), Some("four"));
    assert_eq!(v[1].get("five").map(String::as_str), Some("six"));
    assert_eq!(v[1].get("seven").map(String::as_str), Some("eight"));
}
