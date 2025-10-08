use std::collections::HashMap;

// 6PBE: Zero-indented sequences in explicit mapping keys
#[test]
fn yaml_6pbe_zero_indented_sequences_in_explicit_keys() {
    let y = "---\n?\n- a\n- b\n:\n- c\n- d\n";
    let m: HashMap<Vec<String>, Vec<String>> = serde_saphyr::from_str(y).expect("failed to parse 6PBE");
    let k = vec!["a".to_string(), "b".to_string()];
    let v = vec!["c".to_string(), "d".to_string()];
    assert_eq!(m.get(&k), Some(&v));
    assert_eq!(m.len(), 1);
}
