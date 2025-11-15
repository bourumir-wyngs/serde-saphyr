// TD5N: Invalid scalar after sequence
// YAML should fail to parse because a standalone scalar follows a top-level sequence without a document separator.

#[test]
fn y_td5n_invalid_scalar_after_sequence_should_error() {
    let y = "- item1\n- item2\ninvalid\n";
    let r: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser unexpectedly accepted invalid scalar after sequence: {:?}", r);
}
