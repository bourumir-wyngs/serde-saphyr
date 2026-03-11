use serde::Deserialize;
use serde_saphyr::Error;
use validator::Validate;

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
fn from_reader_with_options_validate_runs_validator_validation_without_snippets() {
    let yaml = "a: \"\"\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());

    let err =
        serde_saphyr::from_reader_with_options_validate::<_, Root>(reader, Default::default())
            .expect_err("must fail validation");

    assert!(matches!(err, Error::ValidatorError { .. }));

    let rendered = err.to_string();
    assert!(
        rendered.contains("validation error at a:"),
        "expected validation message, got: {rendered}"
    );
    assert!(
        rendered.contains("at line 1, column 4"),
        "expected location, got: {rendered}"
    );
    assert!(
        !rendered.contains("<input>"),
        "expected no snippet rendering, got: {rendered}"
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
    assert!(matches!(err, Error::ValidatorError { .. }));

    let rendered = err.to_string();
    assert!(
        rendered.contains("validation error at a:"),
        "expected validation message, got: {rendered}"
    );
    assert!(
        rendered.contains("at line 3, column 4"),
        "expected second-doc location, got: {rendered}"
    );
    assert!(
        !rendered.contains("<input>"),
        "expected no snippet rendering, got: {rendered}"
    );

    assert!(it.next().is_none(), "iterator must end after an error");
}

#[cfg(feature = "include")]
#[test]
fn from_str_with_options_validate_reports_validator_error_from_included_input() {
    let yaml = "a: !include child.yaml\n";
    let options = serde_saphyr::Options::default().with_include_resolver(
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
    let location = err.location().expect("validator error should expose a location");
    assert_eq!(location.source_id(), 2, "expected included source id, got: {location:?}");

    let rendered = err.to_string();
    assert!(
        rendered.contains("included from here:"),
        "expected include-chain note, got: {rendered}"
    );
    assert!(
        rendered.contains("a: !include child.yaml"),
        "expected include callsite snippet, got: {rendered}"
    );
}
