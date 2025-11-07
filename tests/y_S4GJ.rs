// S4GJ: Invalid text after block scalar indicator (folded scalar followed by inline text)
// This YAML is invalid; the parser should report an error.
use serde_json::Value;

#[test]
fn yaml_s4gj_invalid_text_after_block_scalar_indicator_should_fail() {
    let y = "---\nfolded: > first line\n  second line\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted invalid block scalar with trailing text: {:?}", r);
}
