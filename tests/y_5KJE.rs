// 5KJE: Flow Sequence
#[test]
fn yaml_5kje_flow_sequence() {
    let y = "- [ one, two, ]\n- [three ,four]\n";
    let v: Vec<Vec<String>> = serde_saphyr::from_str(y).expect("failed to parse 5KJE");
    let expected: Vec<Vec<String>> = vec![
        vec!["one".to_string(), "two".to_string()],
        vec!["three".to_string(), "four".to_string()],
    ];
    assert_eq!(v, expected);
}
