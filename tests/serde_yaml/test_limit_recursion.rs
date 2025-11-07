use serde_json::Value;
use serde_saphyr::Budget;

#[test]
fn test_recursion_limit_exceeded() {
    let depth = Budget::default().max_depth + 1;
    let yaml = "[".repeat(depth) + &"]".repeat(depth);
    let err = serde_saphyr::from_str::<Value>(&yaml).unwrap_err();
    assert!(
        err.to_string().starts_with("recursion limit exceeded"),
        "unexpected error: {}",
        err
    );
}
