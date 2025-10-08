use std::collections::HashMap;

// 5T43: Colon at the beginning of adjacent flow scalar
#[test]
fn yaml_5t43_colon_at_beginning_of_adjacent_flow_scalar() {
    let y = "- { \"key\":value }\n- { \"key\"::value }\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse 5T43");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].get("key").map(String::as_str), Some("value"));
    assert_eq!(v[1].get("key").map(String::as_str), Some(":value"));
}