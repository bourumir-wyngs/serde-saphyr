// BF9H: Trailing comment in multiline plain scalar — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_bf9h_trailing_comment_in_multiline_plain_scalar_should_fail() {
    let y = "---\nplain: a\n       b # end of scalar\n       c\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "BF9H should fail due to comment splitting a multiline plain scalar");
}