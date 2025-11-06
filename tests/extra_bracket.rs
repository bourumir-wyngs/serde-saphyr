// saphyr-parser 0.0.6 does not emit closing event.
#[ignore]
#[test]
fn debug_extra_bracket_should_err() {
    let y = "---\n[ a, b, c ] ]\n";
    let result: Result<Vec<String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "expected error, got Ok: {:?}", result.ok());
}
