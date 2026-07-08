use serde_json::Value;
use serde_saphyr::{Error, ExternalMessageSource};

#[test]
fn test_recursion_limit_exceeded() {
    let depth = 1_000;
    let yaml = "[".repeat(depth) + &"]".repeat(depth);
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: depth + 1,
        },
    };

    let err = serde_saphyr::from_str_with_options::<Value>(&yaml, options).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        Error::ExternalMessage {
            source: ExternalMessageSource::Parser,
            msg,
            ..
        } if msg == "recursion limit exceeded"
    ));
}
