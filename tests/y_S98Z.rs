use serde_json::Value;

// S98Z: Block scalar with more spaces than first content line (invalid)
// The following YAML is expected to be invalid per test suite.
#[test]
fn yaml_s98z_block_scalar_bad_indentation_should_fail() {
    let y = "empty block scalar: >\n \n  \n   \n  # comment\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted invalid block scalar indentation: {:?}", r);
}
