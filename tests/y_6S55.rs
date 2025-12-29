// 6S55: Invalid scalar at the end of sequence — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_6s55_invalid_scalar_at_end_of_sequence_should_fail() {
    let y = "key:\n - bar\n - baz\n invalid\n";
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Doc {
        key: Vec<String>,
    }
    let result: Result<Doc, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "6S55 should fail to parse due to stray scalar after sequence"
    );
}
