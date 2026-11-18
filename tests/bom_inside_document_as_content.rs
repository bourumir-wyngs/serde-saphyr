#![cfg(feature = "deserialize")]

use serde_json::{Value, json};
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource, Options};

type Parse = fn(&str, Options) -> Result<Value, Error>;

const ENTRYPOINTS: [(&str, Parse); 3] = [
    ("string", |yaml, options| {
        serde_saphyr::from_str_with_options(yaml, options)
    }),
    ("slice", |yaml, options| {
        serde_saphyr::from_slice_with_options(yaml.as_bytes(), options)
    }),
    ("reader", |yaml, options| {
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options)
    }),
];

fn options(bom_inside_document_as_content: bool, with_budget: bool) -> Options {
    let mut options = serde_saphyr::options! {
        bom_inside_document_as_content: bom_inside_document_as_content,
    };
    if !with_budget {
        options.budget = None;
    }
    options
}

fn assert_bom_error(error: &Error) {
    assert!(
        matches!(
            error.without_snippet(),
            Error::ExternalMessage { source, .. }
                if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                    if error.kind() == &ErrorKind::BomInsideDocument)
        ),
        "expected a parser BOM error, got {error:?}"
    );
}

#[test]
fn bom_inside_document_is_rejected_by_default() {
    for (entrypoint, parse) in ENTRYPOINTS {
        let error = parse("key: \u{feff}value\n", Options::default()).expect_err(entrypoint);
        assert_bom_error(&error);
    }
}

#[test]
fn entrypoints_honor_bom_option_with_and_without_budget() {
    for (yaml, expected) in [
        ("---\n\u{feff}key: value\n", json!({"\u{feff}key": "value"})),
        ("key: \u{feff}value\n", json!({"key": "\u{feff}value"})),
        ("key: \u{feff}\n", json!({"key": "\u{feff}"})),
        (
            "[\u{feff}value, other]\n",
            json!(["\u{feff}value", "other"]),
        ),
    ] {
        for (entrypoint, parse) in ENTRYPOINTS {
            for with_budget in [true, false] {
                let error = parse(yaml, options(false, with_budget)).expect_err(entrypoint);
                assert_bom_error(&error);

                assert_eq!(
                    parse(yaml, options(true, with_budget)).expect(entrypoint),
                    expected,
                    "{entrypoint}: {yaml:?}, budget: {with_budget}"
                );
            }
        }
    }
}

#[test]
fn document_prefix_bom_is_skipped_and_scalar_content_is_preserved_in_both_modes() {
    for (yaml, expected) in [
        ("\u{feff}key: value\n", json!({"key": "value"})),
        ("key: va\u{feff}lue\n", json!({"key": "va\u{feff}lue"})),
        ("key: '\u{feff}value'\n", json!({"key": "\u{feff}value"})),
    ] {
        for (entrypoint, parse) in ENTRYPOINTS {
            for enabled in [false, true] {
                assert_eq!(
                    parse(yaml, options(enabled, true)).expect(entrypoint),
                    expected,
                    "{entrypoint}: {yaml:?}, enabled: {enabled}"
                );
            }
        }
    }
}

#[cfg(feature = "include")]
#[test]
fn included_source_prefix_bom_is_skipped_in_both_modes() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    use std::io::Cursor;

    for yaml in [
        "\u{feff}selected: &selected {key: value}\n",
        "\u{feff}---\nselected: &selected {key: value}\n",
    ] {
        for source_kind in ["text", "reader", "utf16le", "utf16be", "anchored"] {
            for enabled in [false, true] {
                let options = options(enabled, true).with_include_resolver(move |req| {
                    let source = match source_kind {
                        "text" => InputSource::from_string(yaml.to_string()),
                        "reader" => InputSource::Reader(Box::new(yaml.as_bytes())),
                        "utf16le" | "utf16be" => {
                            // Encoding the initial U+FEFF supplies the UTF-16 BOM.
                            let bytes: Vec<u8> = yaml
                                .encode_utf16()
                                .flat_map(|unit| {
                                    if source_kind == "utf16le" {
                                        unit.to_le_bytes()
                                    } else {
                                        unit.to_be_bytes()
                                    }
                                })
                                .collect();
                            InputSource::Reader(Box::new(Cursor::new(bytes)))
                        }
                        "anchored" => InputSource::AnchoredText {
                            text: yaml.to_string(),
                            anchor: "selected".to_string(),
                        },
                        _ => unreachable!(),
                    };
                    Ok(ResolvedInclude::new(req.spec, req.spec, source))
                });
                let actual = serde_saphyr::from_str_with_options::<Value>(
                    "before: present\nincluded: !include child.yaml\nafter: present\n",
                    options,
                )
                .expect(source_kind);
                let included = if source_kind == "anchored" {
                    json!({"key": "value"})
                } else {
                    json!({"selected": {"key": "value"}})
                };
                assert_eq!(
                    actual,
                    json!({"before": "present", "included": included, "after": "present"}),
                    "{source_kind}: {yaml:?}, enabled: {enabled}"
                );
            }
        }
    }
}

#[cfg(feature = "include")]
#[test]
fn included_sources_honor_bom_option_with_and_without_budget() {
    use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude};

    const YAML: &str = "selected: &selected {key: \u{feff}value}\n";

    for source_kind in ["text", "reader", "anchored"] {
        for with_budget in [true, false] {
            for enabled in [false, true] {
                let options = options(enabled, with_budget).with_include_resolver(move |req| {
                    let source = match source_kind {
                        "text" => InputSource::from_string(YAML.to_string()),
                        "reader" => InputSource::Reader(Box::new(YAML.as_bytes())),
                        "anchored" => InputSource::AnchoredText {
                            text: YAML.to_string(),
                            anchor: "selected".to_string(),
                        },
                        _ => unreachable!(),
                    };
                    Ok(ResolvedInclude::new(req.spec, req.spec, source))
                });
                let result = serde_saphyr::from_str_with_options::<Value>(
                    "included: !include child.yaml\n",
                    options,
                );

                if enabled {
                    let expected = if source_kind == "anchored" {
                        json!({"included": {"key": "\u{feff}value"}})
                    } else {
                        json!({"included": {"selected": {"key": "\u{feff}value"}}})
                    };
                    assert_eq!(result.expect(source_kind), expected);
                } else {
                    let error = result.expect_err(source_kind);
                    if source_kind == "anchored" {
                        // Anchor collection wraps parser errors in resolver messages.
                        assert!(matches!(
                            error.without_snippet(),
                            Error::ResolverError {
                                error: IncludeResolveError::Message(message),
                                ..
                            } if message.contains("failed to parse include fragment 'selected'")
                        ));
                    } else {
                        assert_bom_error(&error);
                    }
                }
            }
        }
    }
}
