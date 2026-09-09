// 9C9N: Wrong indented flow sequence — marked fail: true
// Rejection is intentionally disabled: relaxed flow indentation maintains compatibility
// with two major YAML libraries, PyYAML and ruamel.yaml.
#[test]
#[ignore = "Under-indented flow sequences are accepted for compatibility with PyYAML and ruamel.yaml"]
fn yaml_9c9n_wrong_indented_flow_sequence_should_fail() {
    let y = "---\nflow: [a,\nb,\nc]\n";
    let result: Result<std::collections::HashMap<String, Vec<String>>, _> =
        serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "9C9N should fail to parse due to wrong indentation in flow sequence"
    );
}
