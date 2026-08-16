// See https://github.com/acatton/serde-yaml-ng/issues/16 -
// DoS by parsing deeply nested YAML
#[test]
fn test_deep_recursion() {
    const N: usize = 8000;
    let y = r#"{"":["#.repeat(N) + &r"]}".repeat(N);
    assert!(serde_saphyr::from_str::<serde::de::IgnoredAny>(&y, ).is_err());
}
