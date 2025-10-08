// 4HVU: Wrong indentation in Sequence — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_4hvu_wrong_indentation_in_sequence_should_fail() {
    let y = "key:\n   - ok\n   - also ok\n  - wrong\n"; // simulate wrong indentation on last item
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Doc { key: Vec<String> }
    let result: Result<Doc, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "4HVU should fail to parse due to wrong indentation in sequence");
}
