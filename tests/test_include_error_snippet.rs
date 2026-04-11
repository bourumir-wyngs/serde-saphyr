#![cfg(all(feature = "serialize", feature = "deserialize"))]
#![cfg(feature = "include")]

use serde::Deserialize;
use serde_saphyr::{
    IncludeRequest, IncludeResolveError, ResolvedInclude, from_reader_with_options,
    from_str_with_options, with_deserializer_from_reader_with_options,
    with_deserializer_from_str_with_options,
};
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
                Ok(ResolvedInclude {
                    id: "included.yaml".to_string(),
                    name: "included.yaml".to_string(),
                    source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                })
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
                Ok(ResolvedInclude {
                    id: "included.yaml".to_string(),
                    name: "included.yaml".to_string(),
                    source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                })
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
                    Ok(ResolvedInclude {
                        id: "included.yaml".to_string(),
                        name: "included.yaml".to_string(),
                        source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                    })
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
fn reader_root_include_site_snippet_uses_snapshot_start_line() {
    let mut main_yaml = String::new();
    for i in 1..50 {
        main_yaml.push_str(&format!("pad{i}: ok\n"));
    }
    main_yaml.push_str("b: !include included.yaml\n");

    let included_yaml = "\nstring\n";
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "included.yaml" {
                Ok(ResolvedInclude {
                    id: "included.yaml".to_string(),
                    name: "included.yaml".to_string(),
                    source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                })
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
