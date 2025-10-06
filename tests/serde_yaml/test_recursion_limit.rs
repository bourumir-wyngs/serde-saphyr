use serde_saphyr::Value;

#[test]
fn test_recursion_limit_exceeded() {
    let depth = 129;
    let yaml = "[".repeat(depth) + &"]".repeat(depth);
    let err = serde_saphyr::from_str::<Value>(&yaml).unwrap_err();
    assert!(
        err.to_string().starts_with("recursion limit exceeded"),
        "unexpected error: {}",
        err
    );
}
