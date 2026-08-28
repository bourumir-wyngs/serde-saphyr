use indoc::indoc;
use serde_json::Value;
use serde_saphyr::budget::BudgetBreach;
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

#[test]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "wasi", target_env = "p1"),
    ignore = "wasm32-wasip1 does not support the large-stack thread required by this test"
)]
fn custom_max_depth_raises_the_parser_block_nesting_limit() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let max_depth = 256;
            let options = serde_saphyr::options! {
                budget: serde_saphyr::budget! {
                    max_depth: max_depth,
                },
            };

            let allowed = format!("{}0", "- ".repeat(max_depth));
            serde_saphyr::from_str_with_options::<Value>(&allowed, options.clone())
                .expect("configured block depth above the parser default should be accepted");

            let excessive = format!("{}0", "- ".repeat(max_depth + 1));
            let error =
                serde_saphyr::from_str_with_options::<Value>(&excessive, options).unwrap_err();
            assert!(matches!(
                error.without_snippet(),
                Error::Budget {
                    breach: BudgetBreach::Depth { depth },
                    ..
                } if *depth == max_depth + 1
            ));
        })
        .expect("deep nesting test thread should spawn")
        .join()
        .expect("deep nesting test thread should not panic");
}
