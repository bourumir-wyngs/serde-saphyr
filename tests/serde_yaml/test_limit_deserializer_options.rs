use indoc::indoc;
use serde_json::Value;

#[test]
fn custom_recursion_limit_exceeded() {
    let depth = 3;
    let yaml = "[".repeat(depth) + &"]".repeat(depth);

    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: 2,
        },
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
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_aliases: 2,
        },
    };
    let result = serde_saphyr::from_str_with_options::<Value>(yaml, options);
    assert!(result.is_err());
}
