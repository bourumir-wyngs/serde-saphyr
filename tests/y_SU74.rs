use serde_json::Value;

// SU74: Anchor and alias used as a mapping key. Marked fail: true in suite.
#[test]
fn y_su74_anchor_and_alias_as_key_should_fail() {
    let y = "key1: &alias value1\n&b *alias : value2\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(
        r.is_err(),
        "Parser accepted anchor+alias used as a mapping key: {:?}",
        r
    );
}
