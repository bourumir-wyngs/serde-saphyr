use indoc::indoc;
use serde_json::Value;
use serde_saphyr::{Budget, Options};

#[test]
fn custom_recursion_limit_exceeded() {
    let depth = 3;
    let yaml = "[".repeat(depth) + &"]".repeat(depth);

    let options = Options {
        budget: Some(Budget {
            max_depth: 2,
            ..Budget::default()
        }),
        ..Options::default()
    };
    let result = serde_saphyr::from_str_with_options::<Value>(&yaml, options);
    assert!(result.is_err());
}

#[test]
fn custom_alias_limit_exceeded() {
    let yaml = indoc! {
        "
        first: &a 1
        second: [*a, *a, *a]
        "
    };
    let options = Options {
        budget: Some(Budget {
            max_aliases: 2,
            ..Budget::default()
        }),
        ..Options::default()
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(result.is_err());
}
