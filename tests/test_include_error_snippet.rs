#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "include")]

use serde::Deserialize;
use serde_saphyr::{
    IncludeRequest, IncludeResolveError, ResolvedInclude, from_reader_with_options,
    from_str_with_options, with_deserializer_from_reader_with_options,
    with_deserializer_from_str_with_options,
};
use std::fmt::Write as _;
use std::io::Cursor;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Config {
    a: String,
    b: usize,
    c: String,
}

#[test]
fn test_include_error_snippet() {
    let main_yaml = r#"
      a: one
      b: !include included.yaml
      c: three
"#;
    let included_yaml = "\nstring\n";

    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "included.yaml" {
                Ok(ResolvedInclude::new(
                    "included.yaml",
                    "included.yaml",
                    serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                ))
            } else {
                Err(IncludeResolveError::Message(format!(
                    "file not found: {}",
                    req.spec
                )))
            }
        },
    );

    let result: Result<Config, _> = from_str_with_options(main_yaml, options);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();

    assert!(err_str.contains("included from here:"));
    assert!(err_str.contains("b: !include included.yaml"));
    assert!(err_str.contains("string"));
}

#[test]
fn test_include_error_snippet_from_reader_with_options() {
    let main_yaml = r#"
      a: one
      b: !include included.yaml
      c: three
"#;
    let included_yaml = "\nstring\n";

    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "included.yaml" {
                Ok(ResolvedInclude::new(
                    "included.yaml",
                    "included.yaml",
                    serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                ))
            } else {
                Err(IncludeResolveError::Message(format!(
                    "file not found: {}",
                    req.spec
                )))
            }
        },
    );

    let reader = Cursor::new(main_yaml.as_bytes());
    let result: Result<Config, _> = from_reader_with_options(reader, options);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("included from here:"));
    assert!(err_str.contains("b: !include included.yaml"));
    assert!(err_str.contains("string"));
}

#[test]
fn test_include_error_snippet_with_deserializer_helpers() {
    let main_yaml = r#"
      a: one
      b: !include included.yaml
      c: three
"#;
    let included_yaml = "\nstring\n";

    let make_options = || {
        serde_saphyr::options! {}.with_include_resolver(
            |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
                if req.spec == "included.yaml" {
                    Ok(ResolvedInclude::new(
                        "included.yaml",
                        "included.yaml",
                        serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                    ))
                } else {
                    Err(IncludeResolveError::Message(format!(
                        "file not found: {}",
                        req.spec
                    )))
                }
            },
        )
    };

    let str_result: Result<Config, _> =
        with_deserializer_from_str_with_options(main_yaml, make_options(), |de| {
            Config::deserialize(de)
        });
    assert!(str_result.is_err());
    let str_err = str_result.unwrap_err().to_string();
    assert!(str_err.contains("included from here:"));
    assert!(str_err.contains("b: !include included.yaml"));
    assert!(str_err.contains("string"));

    let reader = Cursor::new(main_yaml.as_bytes());
    let reader_result: Result<Config, _> =
        with_deserializer_from_reader_with_options(reader, make_options(), |de| {
            Config::deserialize(de)
        });
    assert!(reader_result.is_err());
    let reader_err = reader_result.unwrap_err().to_string();
    assert!(
        reader_err.contains("included from here:"),
        "unexpected reader helper diagnostic: {reader_err}"
    );
    assert!(reader_err.contains("b: !include included.yaml"));
    assert!(reader_err.contains("string"));
}

#[test]
fn reader_backed_include_error_does_not_render_root_lines() {
    // A reader-backed include keeps no text for snippets. Its error location (line 3 of the
    // included source) must not be rendered against line 3 of the root document.
    let main_yaml = "a: root line one\nc: ROOT LINE TWO\nd: ROOT LINE THREE\nx: four\ny: five\nz: six\nb: !include included.yaml\n";
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            assert_eq!(req.spec, "included.yaml");
            Ok(ResolvedInclude::new(
                "included.yaml",
                "included.yaml",
                serde_saphyr::InputSource::from_reader(Cursor::new(b"# c\n# d\nb:   x\n".to_vec())),
            ))
        },
    );

    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Inner {
        b: usize,
    }
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Root {
        b: Inner,
    }

    let err = from_str_with_options::<Root>(main_yaml, options).unwrap_err();
    let location = err.location().expect("error should carry a location");
    assert_eq!((location.line(), location.column()), (3, 6));
    let err_str = err.to_string();
    assert!(
        !err_str.contains("ROOT LINE THREE"),
        "included-source location rendered against the root text: {err_str}"
    );
    assert!(
        err_str.contains("included from here:") && err_str.contains("b: !include included.yaml"),
        "include site should still be shown: {err_str}"
    );
    #[cfg(feature = "miette")]
    {
        let report = serde_saphyr::miette::to_miette_report(&err, main_yaml, "root.yaml");
        let labels = report.labels().map_or(0, Iterator::count);
        assert_eq!(
            labels, 0,
            "miette labelled root text for an included-source location"
        );
    }

    // Errors in the root document itself keep their snippet.
    let options = serde_saphyr::options! {}.with_include_resolver(
        |_req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> { unreachable!() },
    );
    let err = from_reader_with_options::<_, Root>(Cursor::new("b:\n  b: ROOT VALUE\n"), options)
        .unwrap_err();
    assert!(err.to_string().contains("ROOT VALUE"), "{err}");
}

#[test]
fn reader_root_include_site_snippet_uses_snapshot_start_line() {
    let mut main_yaml = String::new();
    for i in 1..50 {
        let _ = writeln!(main_yaml, "pad{i}: ok");
    }
    main_yaml.push_str("b: !include included.yaml\n");

    let included_yaml = "\nstring\n";
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "included.yaml" {
                Ok(ResolvedInclude::new(
                    "included.yaml",
                    "included.yaml",
                    serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                ))
            } else {
                Err(IncludeResolveError::Message(format!(
                    "file not found: {}",
                    req.spec
                )))
            }
        },
    );

    let result: Result<Config, _> = from_reader_with_options(Cursor::new(main_yaml), options);
    assert!(result.is_err());

    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("included from here:"),
        "unexpected diagnostic: {err_str}"
    );
    assert!(
        err_str.contains("--> input:50:"),
        "root include-site snippet should use absolute line 50: {err_str}"
    );
    assert!(
        err_str.contains("b: !include included.yaml"),
        "unexpected diagnostic: {err_str}"
    );
    assert!(
        err_str.contains("--> included.yaml:"),
        "primary snippet should still point at the included source: {err_str}"
    );
}
