use std::collections::HashMap;

// J5UC: Multiple Pair Block Mapping
#[test]
fn yaml_j5uc_multiple_pair_block_mapping() {
    let y = r#"foo: blue
bar: arrr
baz: jazz
"#;
    let v: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse J5UC");
    assert_eq!(v.len(), 3);
    assert_eq!(v.get("foo").map(String::as_str), Some("blue"));
    assert_eq!(v.get("bar").map(String::as_str), Some("arrr"));
    assert_eq!(v.get("baz").map(String::as_str), Some("jazz"));
}
