#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{
    IncludeResolveError, InputSource, Options, ResolvedInclude, from_str_with_options,
};

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
    Options::default().with_include_resolver(
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            let s = req.spec;
            if s == "foo.yaml" {
                Ok(ResolvedInclude {
                    id: s.to_string(),
                    name: s.to_string(),
                    source: InputSource::Text("bar: !include bar.yaml\n".to_string()),
                })
            } else if s == "bar.yaml" {
                Ok(ResolvedInclude {
                    id: s.to_string(),
                    name: s.to_string(),
                    source: InputSource::Text(bar_source.to_string()),
                })
            } else {
                Err(IncludeResolveError::Message("File not found".to_string()))
            }
        },
    )
}

#[cfg(feature = "include")]
fn nested_scalar_include_options(bar_source: &'static str) -> Options {
    Options::default().with_include_resolver(
        move |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            let s = req.spec;
            if s == "foo.yaml" {
                Ok(ResolvedInclude {
                    id: s.to_string(),
                    name: s.to_string(),
                    source: InputSource::Text("!include bar.yaml\n".to_string()),
                })
            } else if s == "bar.yaml" {
                Ok(ResolvedInclude {
                    id: s.to_string(),
                    name: s.to_string(),
                    source: InputSource::Text(bar_source.to_string()),
                })
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

    let options = Options::default().with_include_resolver(
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
