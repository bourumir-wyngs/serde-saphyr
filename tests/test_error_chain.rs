#![cfg(all(feature = "serialize", feature = "deserialize"))]
#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{
    IncludeResolveError, InputSource, Options, ResolvedInclude, from_str_with_options,
};

#[test]
fn snippet_wrappers_do_not_repeat_messages_in_error_chains() {
    for with_snippet in [false, true] {
        let options = serde_saphyr::options! { with_snippet: with_snippet };
        for error in [
            serde_saphyr::from_str_with_options::<u16>("nope", options.clone()).unwrap_err(),
            serde_saphyr::from_str_with_options::<String>("[", options.clone()).unwrap_err(),
            serde_saphyr::from_str_with_options::<&str>(r#""hello\nworld""#, options).unwrap_err(),
        ] {
            let rendered = error.to_string();
            let report = anyhow::Error::new(error);
            assert_eq!(
                format!("{report:#}"),
                rendered,
                "following source() must not append the same diagnostic again"
            );
        }
    }
}

#[test]
fn snippet_wrappers_forward_the_underlying_io_error() {
    for snippet_layers in 0..=2 {
        let mut error = serde_saphyr::Error::IOError {
            cause: std::io::Error::from_raw_os_error(12_345),
        };
        for _ in 0..snippet_layers {
            error = serde_saphyr::Error::WithSnippet {
                regions: Vec::new(),
                crop_radius: 0,
                error: Box::new(error),
            };
        }
        let source = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .expect("source must expose the original I/O error through snippet wrappers");
        assert_eq!(source.raw_os_error(), Some(12_345));
        let expected = format!("IO error: {source}");
        let report = anyhow::Error::new(error);
        assert_eq!(report.chain().count(), 2);
        assert_eq!(format!("{report:#}"), expected);
    }
}

#[test]
fn alias_error_source_is_preserved_through_snippet_wrappers() {
    for with_snippet in [false, true] {
        let error = serde_saphyr::from_str_with_options::<(String, u16)>(
            "[&a nope, *a]",
            serde_saphyr::options! { with_snippet: with_snippet },
        )
        .unwrap_err();
        let rendered = error.to_string();
        let expected = if with_snippet {
            format!(
                "{rendered}: {}: invalid u16 at line 1, column 5",
                error.without_snippet()
            )
        } else {
            format!("{rendered}: invalid u16 at line 1, column 5")
        };
        let report = anyhow::Error::new(error);
        assert_eq!(report.chain().count(), if with_snippet { 3 } else { 2 });
        assert_eq!(format!("{report:#}"), expected);
    }
}

#[test]
fn nested_snippet_wrappers_preserve_the_complete_alias_chain() {
    let error = serde_saphyr::Error::WithSnippet {
        regions: Vec::new(),
        crop_radius: 0,
        error: Box::new(serde_saphyr::from_str::<(String, u16)>("[&a nope, *a]").unwrap_err()),
    };
    let report = anyhow::Error::new(error);
    let variants: Vec<_> = report
        .chain()
        .map(|error| match error.downcast_ref::<serde_saphyr::Error>() {
            Some(serde_saphyr::Error::WithSnippet { .. }) => "WithSnippet",
            Some(serde_saphyr::Error::AliasError { .. }) => "AliasError",
            Some(serde_saphyr::Error::InvalidScalar { .. }) => "InvalidScalar",
            other => panic!("unexpected error in alias chain: {other:?}"),
        })
        .collect();
    assert_eq!(
        variants,
        ["WithSnippet", "WithSnippet", "AliasError", "InvalidScalar"]
    );
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Foo {
    bar: String,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: Foo,
}

#[cfg(feature = "include")]
fn nested_include_options(bar_source: &'static str) -> Options {
    serde_saphyr::options! {}.with_include_resolver(
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            let s = req.spec;
            if s == "foo.yaml" {
                Ok(ResolvedInclude::new(
                    s,
                    s,
                    InputSource::Text("bar: !include bar.yaml\n".to_string()),
                ))
            } else if s == "bar.yaml" {
                Ok(ResolvedInclude::new(
                    s,
                    s,
                    InputSource::Text(bar_source.to_string()),
                ))
            } else {
                Err(IncludeResolveError::Message("File not found".to_string()))
            }
        },
    )
}

#[cfg(feature = "include")]
fn nested_scalar_include_options(bar_source: &'static str) -> Options {
    serde_saphyr::options! {}.with_include_resolver(
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            let s = req.spec;
            if s == "foo.yaml" {
                Ok(ResolvedInclude::new(
                    s,
                    s,
                    InputSource::Text("!include bar.yaml\n".to_string()),
                ))
            } else if s == "bar.yaml" {
                Ok(ResolvedInclude::new(
                    s,
                    s,
                    InputSource::Text(bar_source.to_string()),
                ))
            } else {
                Err(IncludeResolveError::Message("File not found".to_string()))
            }
        },
    )
}

#[cfg(feature = "include")]
#[test]
fn test_error_chain() {
    let yaml = "foo: !include foo.yaml\n";

    let options = nested_include_options("]\n");

    let res = from_str_with_options::<Config>(yaml, options);

    let err = res.unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("bar.yaml"));
    assert!(err_msg.contains("foo.yaml"));
}

#[cfg(feature = "include")]
#[test]
fn test_included_yaml_syntax_error_renders_included_snippet() {
    let yaml = "foo: !include foo.yaml\n";
    let options = nested_scalar_include_options("'unclosed\n");

    let err = from_str_with_options::<Config>(yaml, options).unwrap_err();
    let rendered = err.to_string();

    assert!(
        rendered.contains("bar.yaml"),
        "expected included file name in rendered error, got:\n{rendered}"
    );
    assert!(
        rendered.contains("foo.yaml"),
        "expected include chain to mention parent include, got:\n{rendered}"
    );
    assert!(
        rendered.contains("'unclosed"),
        "expected snippet from included YAML syntax error, got:\n{rendered}"
    );
}

#[cfg(feature = "include")]
#[test]
fn test_included_validation_error_renders_included_snippet() {
    let yaml = "foo: !include foo.yaml\n";
    let options = nested_scalar_include_options("{ nested: true }\n");

    let err = from_str_with_options::<Config>(yaml, options).unwrap_err();
    let rendered = err.to_string();

    assert!(
        rendered.contains("bar.yaml"),
        "expected included file name in rendered error, got:\n{rendered}"
    );
    assert!(
        rendered.contains("{ nested: true }"),
        "expected snippet from included validation error, got:\n{rendered}"
    );
}

#[cfg(feature = "include")]
#[test]
fn test_resolve_error_chain() {
    let yaml = "foo: !include missing.yaml\n";

    let options = serde_saphyr::options! {}.with_include_resolver(
        |_req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            Err(IncludeResolveError::Message("File not found".to_string()))
        },
    );

    let res = from_str_with_options::<Config>(yaml, options);

    let err = res.unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("missing.yaml"));
    assert!(err_msg.contains("File not found"));
}

#[cfg(all(feature = "include", feature = "miette"))]
#[test]
fn test_miette_report_for_nested_include_chain_has_related_include_entries() {
    let yaml = "foo: !include foo.yaml\n";
    let options = nested_scalar_include_options("'broken\n");

    let err = from_str_with_options::<Config>(yaml, options).expect_err("syntax error expected");
    let report = serde_saphyr::miette::to_miette_report(&err, yaml, "root.yaml");
    let rendered = format!("{report:?}");

    assert!(rendered.contains("included from here"), "{rendered}");
    assert!(rendered.contains("foo.yaml"), "{rendered}");
    assert!(rendered.contains("bar.yaml"), "{rendered}");
}
