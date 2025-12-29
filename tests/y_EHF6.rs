use std::collections::HashMap;

// EHF6: Tags for Flow Objects: !!map { k: !!seq [ a, !!str b ] }
// Expectation: tags indicate node types but payload is a mapping with key "k"
// and value being a sequence ["a", "b"].
#[test]
fn yaml_ehf6_tags_for_flow_objects() {
    let y = r#"!!map { k: !!seq [ a, !!str b] }"#;
    let v: HashMap<String, Vec<String>> =
        serde_saphyr::from_str(y).expect("failed to parse EHF6: flow mapping/sequence with tags");
    assert_eq!(v.len(), 1);
    let seq = v.get("k").expect("key 'k' not found");
    assert_eq!(seq, &vec!["a".to_string(), "b".to_string()]);
}
