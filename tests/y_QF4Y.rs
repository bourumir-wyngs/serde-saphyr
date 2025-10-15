use std::collections::BTreeMap;

// QF4Y: Spec Example 7.19. Single Pair Flow Mappings
// YAML: a sequence with a single element which is a single-pair flow mapping.
// We parse into Vec<BTreeMap<String, String>> and assert the contents.
#[test]
fn yaml_qf4y_single_pair_flow_mapping_in_sequence() {
    let y = r#"[
foo: bar
]"#;

    let docs: Vec<BTreeMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse QF4Y");
    assert_eq!(docs.len(), 1);
    let m = &docs[0];
    assert_eq!(m.get("foo").map(String::as_str), Some("bar"));
}
