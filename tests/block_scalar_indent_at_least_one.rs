#![cfg(feature = "deserialize")]

use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource, Options};

type Parse = fn(&str, Options) -> Result<String, Error>;

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

fn options(enabled: bool, with_budget: bool) -> Options {
    let mut options = serde_saphyr::options! { block_scalar_indent_at_least_one: enabled };
    if !with_budget {
        options.budget = None;
    }
    options
}

#[test]
fn column_zero_content_is_allowed_by_default_and_scanned_separately_when_enabled() {
    for header in ["|", ">"] {
        for (body, expected_error) in [
            ("foo\n", ErrorKind::ExpectedDocumentStart),
            ("@\n", ErrorKind::UnexpectedCharacter { character: '@' }),
        ] {
            let yaml = format!("{header}\n{body}");
            for (entrypoint, parse) in ENTRYPOINTS {
                assert_eq!(parse(&yaml, Options::default()).expect(entrypoint), body);
                for with_budget in [true, false] {
                    assert_eq!(
                        parse(&yaml, options(false, with_budget)).expect(entrypoint),
                        body
                    );

                    let error = parse(&yaml, options(true, with_budget)).expect_err(entrypoint);
                    assert!(
                        matches!(
                            error.without_snippet(),
                            Error::ExternalMessage { source, .. }
                                if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                                    if error.kind() == &expected_error)
                        ),
                        "{entrypoint}: {yaml:?}, budget: {with_budget}: {error:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn column_zero_comment_ends_the_scalar_when_enabled() {
    for header in ["|", ">"] {
        let yaml = format!("{header}\n# outside\n");
        for (entrypoint, parse) in ENTRYPOINTS {
            assert_eq!(
                parse(&yaml, options(false, true)).expect(entrypoint),
                "# outside\n"
            );
            assert_eq!(parse(&yaml, options(true, true)).expect(entrypoint), "");
        }
    }
}

#[test]
fn indented_content_and_explicit_indentation_are_preserved() {
    for header in ["|", ">"] {
        for yaml in [
            format!("{header}\n foo\n"),
            format!("{header}1\n foo\n"),
            format!("{header}2\n  foo\n"),
        ] {
            for (entrypoint, parse) in ENTRYPOINTS {
                for enabled in [false, true] {
                    assert_eq!(
                        parse(&yaml, options(enabled, true)).expect(entrypoint),
                        "foo\n"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "include")]
#[test]
fn included_sources_honor_the_option() {
    use serde_saphyr::{InputSource, ResolvedInclude};

    for header in ["|", ">"] {
        for source_kind in ["text", "reader", "anchored"] {
            for with_budget in [true, false] {
                for enabled in [false, true] {
                    let yaml = format!("&selected {header}\n# outside\n");
                    let options = options(enabled, with_budget).with_include_resolver(move |req| {
                        let source = match source_kind {
                            "text" => InputSource::from_string(yaml.clone()),
                            "reader" => {
                                InputSource::Reader(Box::new(std::io::Cursor::new(yaml.clone())))
                            }
                            "anchored" => InputSource::AnchoredText {
                                text: yaml.clone(),
                                anchor: "selected".to_string(),
                            },
                            _ => unreachable!(),
                        };
                        Ok(ResolvedInclude::new(req.spec, req.spec, source))
                    });
                    let actual = serde_saphyr::from_str_with_options::<String>(
                        "!include child.yaml\n",
                        options,
                    )
                    .expect(source_kind);
                    assert_eq!(actual, if enabled { "" } else { "# outside\n" });
                }
            }
        }
    }
}
