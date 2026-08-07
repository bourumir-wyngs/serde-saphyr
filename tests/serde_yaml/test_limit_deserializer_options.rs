use indoc::indoc;
use serde_json::Value;
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource};

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

#[test]
fn custom_flow_nesting_limit_is_applied_to_parser() {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: 8,
            flow_nesting_limit: 1,
        },
    };

    let error = serde_saphyr::from_str_with_options::<Value>("[[]]", options).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::ExternalMessage { source, .. }
            if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                if error.kind() == &ErrorKind::RecursionLimitExceeded)
    ));
}
