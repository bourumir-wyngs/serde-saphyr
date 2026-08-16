// See https://github.com/acatton/serde-yaml-ng/issues/16 -
// DoS by parsing deeply nested YAML
use serde_saphyr::budget::BudgetBreach;
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Budget, Error, ExternalMessageSource};

#[test]
fn test_deep_recursion() {
    const N: usize = 8000;
    let y = r#"{"":["#.repeat(N) + &r"]}".repeat(N);

    match serde_saphyr::from_str::<serde::de::IgnoredAny>(&y) {
        // Both serde-saphyr and granit-parser have limiters due historical reasons.
        Err(err) => match err.without_snippet() {
            Error::Budget {
                breach: BudgetBreach::Depth { depth },
                ..
            } if *depth > Budget::default().max_depth => {}
            Error::ExternalMessage { source, .. }
                if matches!(
                    source.as_ref(),
                    ExternalMessageSource::Parser(error)
                        if error.kind() == &ErrorKind::RecursionLimitExceeded
                ) => {}
            unexpected => {
                panic!("expected a budget or parser recursion-limit error, got {unexpected:?}")
            }
        },
        Ok(_) => panic!("expected deeply nested YAML to exceed the parsing budget"),
    }
}
