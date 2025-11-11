use serde_json::Value;

// SY6V: Anchor before sequence entry on same line. Marked fail: true in suite.
#[test]
fn y_sy6v_anchor_before_seq_entry_should_fail() {
    let y = "&anchor - sequence entry\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted an anchor before a sequence entry on the same line: {:?}", r);
}
