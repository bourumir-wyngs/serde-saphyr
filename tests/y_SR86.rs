use serde_json::Value;

// SR86: Anchor plus Alias in value: key2: &b *a
// Parser should reject alias immediately
// following an anchor as part of the same node.
#[test]
fn y_sr86_anchor_plus_alias_should_fail() {
    let y = "key1: &a value\nkey2: &b *a\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted anchor+alias combination in a value: {:?}", r);
}
