// 6M2F: Aliases in Explicit Block Mapping
use std::collections::HashMap;

#[test]
fn yaml_6m2f_aliases_in_explicit_block_mapping() {
    let y = "? &a a\n: &b b\n: *a\n";
    let m: HashMap<Option<String>, String> = serde_saphyr::from_str(y).expect("failed to parse 6M2F");
    assert_eq!(m.get(&Some("a".to_string())).map(String::as_str), Some("b"));
    assert_eq!(m.get(&None).map(String::as_str), Some("a"));
}
