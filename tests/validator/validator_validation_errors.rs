use serde::Deserialize;
use serde_saphyr::Error;
use validator::Validate;

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, Validate)]
struct IncludeValidationRoot {
    #[validate(nested)]
    a: IncludeValidationLeaf,
}

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, Validate)]
struct IncludeValidationLeaf {
    #[validate(length(min = 1))]
    value: String,
}

#[derive(Debug, Deserialize, Validate)]
struct Root {
    #[validate(length(min = 1))]
    a: String,
}

#[test]
fn from_str_with_options_validate_runs_validator_validation() {
    let yaml = "a: \"\"\n";

    let err = serde_saphyr::from_str_with_options_validate::<Root>(yaml, Default::default())
        .expect_err("must fail validation");

    let rendered = err.to_string();
    assert!(
        rendered.contains("validation error"),
        "expected validation error output, got: {rendered}"
    );
    assert!(
        rendered.contains("for `a`"),
        "expected path `a` in output, got: {rendered}"
    );
    assert!(
        rendered.contains("line 1 column 4"),
        "expected location in output, got: {rendered}"
    );
}

#[test]
fn serde_rename() {
    #[derive(Debug, Deserialize, Validate)]
    #[serde(rename_all = "camelCase")]
    struct StyleRenamedRoot {
        #[validate(length(min = 1))]
        my_field: String,
    }

    let yaml = "myField: \"\"\n";

    let err =
        serde_saphyr::from_str_with_options_validate::<StyleRenamedRoot>(yaml, Default::default())
            .expect_err("must fail validation");
    let rendered = err.to_string();

    assert!(
        rendered.contains("for `myField`"),
        "expected resolved leaf name `myField` in output, got: {rendered}"
    );
    assert!(
        rendered.contains("line 1 column 10"),
        "expected location for renamed field, got: {rendered}"
    );
}

#[test]
fn from_multiple_with_options_validate_returns_all_validation_errors() {
    let yaml = "a: \"\"\n---\na: \"\"\n";

    let err = serde_saphyr::from_multiple_with_options_validate::<Root>(yaml, Default::default())
        .expect_err("must fail validation");

    let Error::ValidatorErrors { errors } = &err else {
        panic!("expected ValidatorErrors, got: {err:?}");
    };
    assert_eq!(errors.len(), 2);

    let rendered = err.to_string();
    assert!(
        rendered.contains("line 1 column 4"),
        "expected first document error location, got: {rendered}"
    );
    assert!(
        rendered.contains("line 3 column 4"),
        "expected second document error location, got: {rendered}"
    );
}

#[test]
fn reader_validation_root_snapshot_out_of_range_has_no_incorrect_snippet() {
    let mut yaml = String::new();
    for i in 0..9000 {
        yaml.push_str(&format!("skip_{i}: x\n"));
    }
    yaml.push_str("a: \"\"\n");
    let reader = std::io::Cursor::new(yaml.into_bytes());

    let err =
        serde_saphyr::from_reader_with_options_validate::<_, Root>(reader, Default::default())
            .expect_err("must fail validation");

    match &err {
        Error::ValidatorError { .. } => {}
        Error::WithSnippet { error, .. } if matches!(**error, Error::ValidatorError { .. }) => {}
        other => panic!("expected ValidatorError, got: {other:?}"),
    }

    let rendered = err.to_string();
    assert!(
        rendered.contains("validation error"),
        "expected validation message, got: {rendered}"
    );
    assert!(
        rendered.contains("line 9001 column 4"),
        "expected location, got: {rendered}"
    );
    assert!(
        rendered.contains("9001 | a: \"\""),
        "expected either a correct high-line snippet or no snippet, got: {rendered}"
    );
    assert!(
        !rendered.contains("<input>:1:"),
        "expected no incorrect line-1 snippet rendering, got: {rendered}"
    );
}

#[test]
fn read_with_options_validate_validates_each_document_in_iterator() {
    let yaml = concat!("a: \"ok\"\n", "---\n", "a: \"\"\n",);
    let mut reader = std::io::Cursor::new(yaml.as_bytes());

    let mut it =
        serde_saphyr::read_with_options_validate::<_, Root>(&mut reader, Default::default());

    let first = it
        .next()
        .expect("must yield first document")
        .expect("first doc should be valid");
    assert_eq!(first.a, "ok");

    let err = it
        .next()
        .expect("must yield second document")
        .expect_err("second document must fail validation");
    match &err {
        Error::ValidatorError { .. } => {}
        Error::WithSnippet { error, .. } if matches!(**error, Error::ValidatorError { .. }) => {}
        other => panic!("expected ValidatorError, got: {other:?}"),
    }

    let rendered = err.to_string();
    assert!(
        rendered.contains("validation error"),
        "expected validation message, got: {rendered}"
    );
    assert!(
        rendered.contains("line 3 column 4"),
        "expected second-doc location, got: {rendered}"
    );
    assert!(
        rendered.contains(":3:4"),
        "expected reader snippet location, got: {rendered}"
    );
    assert!(
        rendered.contains("3 | a: \"\""),
        "expected second-doc snippet contents, got: {rendered}"
    );

    assert!(it.next().is_none(), "iterator must end after an error");
}

#[cfg(feature = "include")]
#[test]
fn reader_validator_validation_in_text_include_has_snippet() {
    let yaml = "a: !include child.yaml\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            if req.spec == "child.yaml" {
                Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::from_string("\"\"\n".to_string()),
                })
            } else {
                Err(serde_saphyr::IncludeResolveError::Message("not found".to_string()))
            }
        },
    );

    let err = serde_saphyr::from_reader_with_options_validate::<_, Root>(reader, options)
        .expect_err("included value must fail validator rule");
    match &err {
        Error::ValidatorError { .. } => {}
        Error::WithSnippet { error, .. } if matches!(**error, Error::ValidatorError { .. }) => {}
        other => panic!("expected ValidatorError, got: {other:?}"),
    }

    let location = err
        .location()
        .expect("validator error should expose a location");
    assert_eq!(
        location.source_id(),
        2,
        "expected included source id, got: {location:?}"
    );

    let rendered = err.to_string();
    assert!(
        rendered.contains("| \"\""),
        "expected snippet to render included content, got: {rendered}"
    );
}

#[cfg(feature = "include")]
#[test]
fn from_str_with_options_validate_reports_validator_error_from_included_input() {
    let yaml = "a: !include child.yaml\n";
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            if req.spec == "child.yaml" {
                Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::from_string("\"\"\n".to_string()),
                })
            } else {
                Err(serde_saphyr::IncludeResolveError::Message("not found".to_string()))
            }
        },
    );

    let err = serde_saphyr::from_str_with_options_validate::<Root>(yaml, options)
        .expect_err("included value must fail validator rule");
    match &err {
        Error::ValidatorError { .. } => {}
        Error::WithSnippet { error, .. } if matches!(**error, Error::ValidatorError { .. }) => {}
        other => panic!("expected ValidatorError, got: {other:?}"),
    }
    let location = err
        .location()
        .expect("validator error should expose a location");
    assert_eq!(
        location.source_id(),
        2,
        "expected included source id, got: {location:?}"
    );

    let rendered = err.to_string();
    assert!(
        rendered.contains("| \"\""),
        "expected snippet to render included content, got: {rendered}"
    );
}

#[cfg(feature = "include")]
#[test]
fn validator_multidoc_validation_in_included_file_renders_included_snippet() {
    let yaml = "a:\n  value: ok\n---\na: !include child.yaml\n";
    let options = serde_saphyr::options! {}.with_include_resolver(
        |req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
            match req.spec {
                "child.yaml" => Ok(serde_saphyr::ResolvedInclude {
                    id: req.spec.to_string(),
                    name: req.spec.to_string(),
                    source: serde_saphyr::InputSource::from_string("value: \"\"\n".to_string()),
                }),
                other => Err(serde_saphyr::IncludeResolveError::Message(format!("unexpected include: {other}"))),
            }
        },
    );

    let err =
        serde_saphyr::from_multiple_with_options_validate::<IncludeValidationRoot>(yaml, options)
            .expect_err("included value in second document must fail validator rule");

    let Error::ValidatorErrors { errors } = &err else {
        panic!("expected ValidatorErrors, got: {err:?}");
    };
    assert_eq!(
        errors.len(),
        1,
        "expected one failing document, got: {errors:?}"
    );

    let rendered = err.to_string();
    assert!(
        rendered.contains("--> (defined):1:8"),
        "expected included file content as primary snippet, got: {rendered}"
    );
    assert!(
        rendered.contains("| value: \"\""),
        "expected included content in snippet, got: {rendered}"
    );
    assert!(
        rendered.contains("--> <input>:4:13"),
        "expected second document include-site snippet, got: {rendered}"
    );
}

#[test]
fn from_multiple_validate_uses_default_options() {
    let values = serde_saphyr::from_multiple_validate::<Root>("a: ok\n---\na: still-ok\n").unwrap();

    assert_eq!(values.len(), 2);
    assert_eq!(values[0].a, "ok");
    assert_eq!(values[1].a, "still-ok");
}

#[test]
fn from_slice_validate_runs_validator_validation() {
    let err = serde_saphyr::from_slice_validate::<Root>(b"a: \"\"\n")
        .expect_err("empty string must fail validator validation");

    assert!(
        err.to_string().contains("validation error"),
        "expected validator error output, got: {err}"
    );
}

#[test]
fn from_slice_with_options_validate_rejects_invalid_utf8() {
    let err = serde_saphyr::from_slice_with_options_validate::<Root>(&[0xff], Default::default())
        .expect_err("invalid UTF-8 must be rejected");

    assert!(matches!(err, Error::InvalidUtf8Input));
}

#[test]
fn from_reader_validate_accepts_valid_document() {
    let value = serde_saphyr::from_reader_validate::<_, Root>(std::io::Cursor::new(b"a: ok\n"))
        .expect("valid document should deserialize");

    assert_eq!(value.a, "ok");
}

#[test]
fn read_validate_uses_default_options() {
    let mut reader = std::io::Cursor::new("~\n---\na: ok\n".as_bytes());
    let mut it = serde_saphyr::read_validate::<_, Root>(&mut reader);

    let value = it
        .next()
        .expect("iterator must yield the non-null document")
        .expect("document should be valid");
    assert_eq!(value.a, "ok");
    assert!(it.next().is_none(), "iterator must stop at end of input");
}
