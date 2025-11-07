use serde_json::Value;

// SF5V: Duplicate YAML directive
// The parser should fail when encountering two %YAML directives in a single stream.
#[test]
fn yaml_sf5v_duplicate_yaml_directive_should_fail() {
    let y = "%YAML 1.2\n%YAML 1.2\n---\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted duplicate %YAML directives: {:?}", r);
}
