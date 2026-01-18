use std::collections::HashMap;

#[test]
fn yaml_3gzx_alias_nodes_in_mapping() {
    let yaml = "First occurrence: &anchor Foo\nSecond occurrence: *anchor\nOverride anchor: &anchor Bar\nReuse anchor: *anchor\n";

    let m: HashMap<String, String> =
        serde_saphyr::from_str(yaml).expect("failed to parse 3GZX mapping");
    assert_eq!(m.get("First occurrence").map(String::as_str), Some("Foo"));
    assert_eq!(m.get("Second occurrence").map(String::as_str), Some("Foo"));
    assert_eq!(m.get("Override anchor").map(String::as_str), Some("Bar"));
    assert_eq!(m.get("Reuse anchor").map(String::as_str), Some("Bar"));
    assert_eq!(m.len(), 4);
}
