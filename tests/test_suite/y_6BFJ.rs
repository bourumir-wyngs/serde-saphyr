use std::collections::HashMap;

// 6BFJ: Mapping, key and flow sequence item anchors
// Expect a mapping whose key is the sequence [a, b, c] and value is "value".
#[test]
fn yaml_6bfj_mapping_key_and_item_anchors() {
    let y = "---\n&mapping\n&key [ &item a, b, c ]: value\n";
    let m: HashMap<Vec<String>, String> = serde_saphyr::from_str(y).expect("failed to parse 6BFJ");
    let k = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    assert_eq!(m.get(&k).map(String::as_str), Some("value"));
    assert_eq!(m.len(), 1);
}
