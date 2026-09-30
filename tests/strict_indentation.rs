#![cfg(feature = "deserialize")]

use serde_json::{Value, json};
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource, Options};

const UNDER_INDENTED: &str = "key: &selected [\n  1,\n  2\n]\n";
const CONFORMING: &str = "key: &selected [\n  1,\n  2\n ]\n";

fn options(strict_indentation: bool, with_budget: bool) -> Options {
    let mut options = serde_saphyr::options! { strict_indentation: strict_indentation };
    if !with_budget {
        options.budget = None;
    }
    options
}

fn assert_parser_indentation_error(error: &Error) {
    assert!(
        matches!(
            error.without_snippet(),
            Error::ExternalMessage { source, .. }
                if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                    if error.kind() == &ErrorKind::InvalidIndentation)
        ),
        "expected a parser indentation error, got {error:?}"
    );
}

#[test]
fn reader_and_slice_honor_strict_indentation_with_and_without_budget() {
    type Parse = fn(&str, Options) -> Result<Value, Error>;
    let entrypoints: [(&str, Parse); 2] = [
        ("slice", |yaml, options| {
            serde_saphyr::from_slice_with_options(yaml.as_bytes(), options)
        }),
        ("reader", |yaml, options| {
            serde_saphyr::from_reader_with_options(yaml.as_bytes(), options)
        }),
    ];

    for (entrypoint, parse) in entrypoints {
        for with_budget in [true, false] {
            for strict_indentation in [false, true] {
                let result = parse(UNDER_INDENTED, options(strict_indentation, with_budget));
                if strict_indentation {
                    assert_parser_indentation_error(&result.expect_err(entrypoint));
                } else {
                    assert_eq!(result.expect(entrypoint), json!({"key": [1, 2]}));
                }

                assert_eq!(
                    parse(CONFORMING, options(strict_indentation, with_budget)).expect(entrypoint),
                    json!({"key": [1, 2]})
                );
            }
        }
    }
}

#[cfg(feature = "include")]
#[test]
fn included_sources_honor_strict_indentation_with_and_without_budget() {
    use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude};

    for source_kind in ["text", "reader", "anchored"] {
        for with_budget in [true, false] {
            for strict_indentation in [false, true] {
                for yaml in [UNDER_INDENTED, CONFORMING] {
                    let options = options(strict_indentation, with_budget).with_include_resolver(
                        move |req| {
                            let source = match source_kind {
                                "text" => InputSource::from_string(yaml.to_string()),
                                "reader" => InputSource::Reader(Box::new(yaml.as_bytes())),
                                "anchored" => InputSource::AnchoredText {
                                    text: yaml.to_string(),
                                    anchor: "selected".to_string(),
                                },
                                _ => unreachable!(),
                            };
                            Ok(ResolvedInclude::new(req.spec, req.spec, source))
                        },
                    );
                    let result = serde_saphyr::from_str_with_options::<Value>(
                        "included: !include child.yaml\n",
                        options,
                    );

                    if strict_indentation && yaml == UNDER_INDENTED {
                        let error = result.expect_err(source_kind);
                        if source_kind == "anchored" {
                            // Anchor collection wraps scanner errors in resolver messages.
                            assert!(matches!(
                                error.without_snippet(),
                                Error::ResolverError {
                                    error: IncludeResolveError::Message(message),
                                    ..
                                } if message.contains("failed to scan include fragment 'selected'")
                                    && message.contains("invalid indentation")
                            ));
                        } else {
                            assert_parser_indentation_error(&error);
                        }
                    } else {
                        let expected = if source_kind == "anchored" {
                            json!({"included": [1, 2]})
                        } else {
                            json!({"included": {"key": [1, 2]}})
                        };
                        assert_eq!(result.expect(source_kind), expected);
                    }
                }
            }
        }
    }
}
