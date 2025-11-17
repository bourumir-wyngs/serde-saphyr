use serde_json::Value;

// SU5Z: Comment without whitespace after doublequoted scalar
// YAML requires whitespace before a comment (#).
#[test]
fn y_su5z_comment_without_whitespace_should_fail() {
    let y = "key: \"value\"# invalid comment\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted a comment without preceding whitespace after a double-quoted scalar: {:?}", r);
}
