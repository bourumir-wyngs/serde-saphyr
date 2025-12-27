#![cfg(feature = "garde")]

use garde::Validate;
use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize, Validate)]
struct Root {
    #[garde(length(min = 1))]
    a: String,
}

#[derive(Debug, Deserialize, Validate)]
#[allow(dead_code)]
struct AnchorRoot {
    // Just defined here
    #[garde(skip)]
    a: String,
    #[garde(length(min = 2))]
    b: String,
}

#[derive(Debug, Deserialize, Validate)]
#[allow(dead_code)]
struct NestedAnchorRoot {
    #[garde(dive)]
    outer: Outer,
}

#[derive(Debug, Deserialize, Validate)]
#[allow(dead_code)]
struct Outer {
    #[garde(dive)]
    inner: Inner,
}

#[derive(Debug, Deserialize, Validate)]
#[allow(dead_code)]
struct Inner {
    // Just defined here
    #[garde(skip)]
    a: String,
    #[garde(length(min = 2))]
    b: String,
}

#[test]
fn from_str_with_options_valid_runs_garde_validation() {
    let yaml = "a: \"\"\n";

    let err = serde_saphyr::from_str_with_options_valid::<Root>(yaml, Default::default())
        .expect_err("must fail validation");

    let rendered = err.to_string();

    let expected = concat!(
        "error: line 1 column 4: validation error: length is lower than 1 for `a`\n",
        " --> (defined):1:4\n",
        "  |\n",
        "1 | a: \"\"\n",
        "  |    ^ validation error: length is lower than 1 for `a`",
    );
    //println!("{rendered}");

    assert_eq!(rendered, expected);
}

#[test]
fn from_str_validated_converts_garde_report_into_error() {
    let yaml = "a: \"\"\n";

    let err = serde_saphyr::from_str_valid::<Root>(yaml).expect_err("must fail validation");

    let rendered = err.to_string();

    // Default options enable snippet wrapping.
    match &err {
        serde_saphyr::Error::WithSnippet { error, .. } => {
            assert!(matches!(
                **error,
                serde_saphyr::Error::ValidationError { .. }
            ));
        }
        serde_saphyr::Error::ValidationError { .. } => {}
        other => panic!("expected validation error, got: {other:?}"),
    }
    assert!(rendered.contains("defined"), "expected snippet output, got: {rendered}");
 }

#[test]
fn from_multiple_with_options_valid_returns_all_validation_errors() {
    // Two documents; both fail the same `garde` constraint.
    // Locations are relative to the whole YAML stream.
    let yaml = "a: \"\"\n---\na: \"\"\n";

    let err = serde_saphyr::from_multiple_with_options_valid::<Root>(yaml, Default::default())
        .expect_err("must fail validation");

    let Error::ValidationErrors { errors } = &err else {
        panic!("expected ValidationErrors, got: {err:?}");
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
fn validation_error_shows_referenced_and_defined_snippets_for_aliases() {
    // `b` is an alias of `a`. For `b`, garde path-to-location recording captures:
    // - referenced: location of the alias token `*A`
    // - defined: location of the anchored scalar value (the `""` under `&A`)
    // Use a non-empty string to avoid it being treated as null-like by any YAML adapters.
    let yaml = "a: &A \"x\"\nb: *A\n";

    let err = serde_saphyr::from_str_with_options_valid::<AnchorRoot>(yaml, Default::default())
        .expect_err("must fail validation");
    let rendered = err.to_string();
    //println!("{rendered}");

    // We want to see the primary (use-site) diagnostic.
    assert!(
        rendered.contains(" --> the value is used here:2:4"),
        "expected use-site snippet header, got: {rendered}"
    );

    // And we want the secondary anchor context rendered as a custom message + a bare snippet
    // window (no `note:` / `defined:` report header).
    assert!(
        rendered.contains("This value comes indirectly from the anchor at line 1 column 7:"),
        "expected anchor context line, got: {rendered}"
    );
    
    // And ensure the failing path is mentioned.
    assert!(
        rendered.contains("for `b`"),
        "expected failing path `b` in output, got: {rendered}"
    );
}

#[test]
fn validation_error_shows_longer_garde_path_for_nested_structures() {
    // Same anchor/alias scenario as `validation_error_shows_referenced_and_defined_snippets_for_aliases`,
    // but nested inside structures so garde produces a longer path like `outer.inner.b`.
    let yaml = concat!(
        "outer:\n",
        "  inner:\n",
        "    a: &A \"x\"\n",
        "    b: *A\n",
    );

    let err = serde_saphyr::from_str_with_options_valid::<NestedAnchorRoot>(yaml, Default::default())
        .expect_err("must fail validation");
    let rendered = err.to_string();
    //println!("{rendered}");

    // Primary use-site snippet.
    assert!(
        rendered.contains(" --> the value is used here:4:8"),
        "expected use-site snippet header, got: {rendered}"
    );

    // Anchor context line should include the definition coordinates.
    assert!(
        rendered.contains("This value comes indirectly from the anchor at line 3 column 11:"),
        "expected anchor context line, got: {rendered}"
    );

    // And ensure we see the longer failing path.
    assert!(
        rendered.contains("for `outer.inner.b`"),
        "expected failing path `outer.inner.b` in output, got: {rendered}"
    );
}

#[test]
fn from_multiple_with_options_valid_validates_each_document() {
    let yaml = concat!(
        "a: \"ok\"\n",
        "---\n",
        "a: \"\"\n",
    );

    let err = serde_saphyr::from_multiple_with_options_valid::<Root>(yaml, Default::default())
        .expect_err("second document must fail validation");
    let rendered = err.to_string();

    // The failure should be attributed to the second document.
    assert!(
        rendered.contains("line 3 column 4"),
        "expected validation error location in second document, got: {rendered}"
    );
    assert!(
        rendered.contains("for `a`"),
        "expected garde path in output, got: {rendered}"
    );
}

#[test]
fn from_reader_with_options_valid_runs_garde_validation_without_snippets() {
    let yaml = "a: \"\"\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());

    let err = serde_saphyr::from_reader_with_options_valid::<_, Root>(reader, Default::default())
        .expect_err("must fail validation");

    // Reader-based API does not have access to the full text, so it must not render snippets.
    assert!(matches!(err, Error::ValidationError { .. }));

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
fn read_with_options_valid_validates_each_document_in_iterator() {
    let yaml = concat!(
        "a: \"ok\"\n",
        "---\n",
        "a: \"\"\n",
    );
    let mut reader = std::io::Cursor::new(yaml.as_bytes());

    let mut it = serde_saphyr::read_with_options_valid::<_, Root>(&mut reader, Default::default());

    let first = it.next().expect("must yield first document").expect("first doc should be valid");
    assert_eq!(first.a, "ok");

    let err = it
        .next()
        .expect("must yield second document")
        .expect_err("second document must fail validation");
    assert!(matches!(err, Error::ValidationError { .. }));

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
