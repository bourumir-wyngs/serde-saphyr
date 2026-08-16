use serde_saphyr::{budget, options};

// See https://github.com/acatton/serde-yaml-ng/issues/16 -
// DoS by parsing deeply nested YAML
#[test]
fn test_deep_recursion() {
    let unlimited = options! {
        budget: budget! {
            flow_nesting_limit: 100,
            max_depth: 1000
        }
    };
    const N: usize = 800;
    let y = r#"{"":["#.repeat(N) + &r"]}".repeat(N);
    assert!(serde_saphyr::from_str_with_options::<serde::de::IgnoredAny>(&y, unlimited).is_err());
}
