// T833: Flow mapping missing a separating comma
// YAML should fail to parse due to missing comma between flow mapping entries.

#[test]
fn y_t833_flow_mapping_missing_comma_should_error() {
    let y = "---\n{\n foo: 1\n bar: 2 }\n";
    let r: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser unexpectedly accepted flow mapping without comma: {:?}", r);
}
