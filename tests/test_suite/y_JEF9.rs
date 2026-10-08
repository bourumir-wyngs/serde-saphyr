// JEF9: Trailing whitespace in streams with |+ block scalar keep
// We validate the simplest case: a sequence with a single kept-empty block scalar
// that contains two blank lines, resulting in "\n\n".

#[test]
fn yaml_jef9_trailing_whitespace_block_keep() {
    let y = r#"- |+


"#;
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse JEF9 first variant");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].as_str(), "\n\n");
}

#[test]
fn yaml_suite_jef9_00() {
    super::yaml_suite_support::assert_json_case("- |+\n\n\n", "[\n  \"\\n\\n\"\n]\n");
}

#[test]
fn yaml_suite_jef9_01() {
    super::yaml_suite_support::assert_json_case("- |+\n   \n", "[\n  \"\\n\"\n]\n");
}

// Upstream JEF9/02 expects ["\n"] even though the final indentation has no line
// break: this variant inherits its tree/json expectations from the preceding one.
// An empty-string interpretation of the YAML 1.2.2 grammar has been proposed, but
// we retain the established 1.3.0 behavior for compatibility with the upstream suite.
// https://github.com/yaml/yaml-test-suite/blob/main/src/JEF9.yaml
#[test]
fn yaml_suite_jef9_02() {
    super::yaml_suite_support::assert_json_case("- |+\n   ", "[\n  \"\\n\"\n]\n");
}
