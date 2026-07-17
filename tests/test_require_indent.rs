#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde_json::Value;
use serde_saphyr::{Error, RequireIndent};

fn parse(require: RequireIndent, yaml: &str) -> Result<Value, Error> {
    let options = serde_saphyr::options! { require_indent: require };
    serde_saphyr::from_str_with_options::<Value>(yaml, options)
}

fn parse_multiple(require: RequireIndent, yaml: &str) -> Result<Vec<Value>, Error> {
    let options = serde_saphyr::options! { require_indent: require };
    serde_saphyr::from_multiple_with_options::<Value>(yaml, options)
}

#[rstest]
#[case::even_two(RequireIndent::Even, "root:\n  child: value\n")]
#[case::divisible_four(RequireIndent::Divisible(4), "root:\n    child: value\n")]
#[case::uniform_inferred(RequireIndent::Uniform(None), "a:\n  b: 1\n  c: 2\n")]
#[case::uniform_two(RequireIndent::Uniform(Some(2)), "x:\n  y:\n    z: 1\n")]
#[case::unchecked(RequireIndent::Unchecked, "a:\n   b:\n       c: 1\n")]
fn accepts_valid_indentation(#[case] require: RequireIndent, #[case] yaml: &str) {
    assert!(parse(require, yaml).is_ok());
}

#[rstest]
#[case::even_rejects_odd(
    RequireIndent::Even,
    "root:\n   child: value\n",
    RequireIndent::Even,
    3
)]
#[case::divisible_four_rejects_two(
    RequireIndent::Divisible(4),
    "root:\n  child: value\n",
    RequireIndent::Divisible(4),
    2
)]
#[case::uniform_rejects_mixed(
    RequireIndent::Uniform(None),
    "a:\n  b:\n     c: 1\n",
    RequireIndent::Uniform(Some(2)),
    5
)]
fn rejects_invalid_indentation(
    #[case] require: RequireIndent,
    #[case] yaml: &str,
    #[case] expected_required: RequireIndent,
    #[case] expected_actual: usize,
) {
    let err = parse(require, yaml).unwrap_err();
    assert_indentation_error(&err, expected_required, expected_actual);
}

#[test]
fn divisible_zero_indent_returns_error() {
    let options = serde_saphyr::options! {
        require_indent: RequireIndent::Divisible(0),
    };
    let err = serde_saphyr::from_str_with_options::<Value>("value\n", options).unwrap_err();
    assert!(matches!(
        err.without_snippet(),
        Error::InvalidOptions { msg, .. }
            if msg == "invalid deserialization options: require_indent Divisible(0) is not allowed; indentation divisor must be non-zero"
    ));
}

#[rstest]
#[case::literal("|")]
#[case::folded(">")]
fn rejects_invalid_non_empty_block_scalar_content_indentation(#[case] marker: &str) {
    let yaml = format!("root:\n  text: {marker}\n   body\n");

    let err = parse(RequireIndent::Even, &yaml).unwrap_err();
    assert_indentation_error(&err, RequireIndent::Even, 3);
}

#[rstest]
#[case::literal("|")]
#[case::folded(">")]
fn accepts_valid_non_empty_block_scalar_content_indentation(#[case] marker: &str) {
    let yaml = format!("root:\n  text: {marker}\n    body\n");

    assert!(parse(RequireIndent::Even, &yaml).is_ok());
}

#[rstest]
#[case::literal("|+")]
#[case::folded(">+")]
fn whitespace_only_block_scalar_content_does_not_set_indentation_unit(#[case] marker: &str) {
    let yaml = format!("empty: {marker}\n   \nnext:\n  value: ok\n");

    assert!(parse(RequireIndent::Uniform(None), &yaml).is_ok());
}

#[rstest]
#[case::literal("|")]
#[case::folded(">")]
fn non_empty_block_scalar_content_sets_uniform_indentation_unit(#[case] marker: &str) {
    let yaml = format!("text: {marker}\n   body\nnext:\n  value: ok\n");

    let err = parse(RequireIndent::Uniform(None), &yaml).unwrap_err();
    assert_indentation_error(&err, RequireIndent::Uniform(Some(3)), 2);
}

// Regression for https://github.com/bourumir-wyngs/serde-saphyr/issues/132.
#[rstest]
#[case::first_indent_too_wide("x:\n   z: 1\n", 3)]
#[case::first_indent_too_narrow("x:\n z: 1\n", 1)]
#[case::inferred_then_off_multiple("x:\n y:\n   z: 1\n", 1)]
#[case::valid_first_then_off("x:\n  y:\n   z: 1\n", 3)]
fn uniform_some_reports_configured_value(#[case] yaml: &str, #[case] found: usize) {
    let err = parse(RequireIndent::Uniform(Some(2)), yaml).unwrap_err();
    assert_indentation_error(&err, RequireIndent::Uniform(Some(2)), found);
}

#[test]
fn default_is_unchecked() {
    let options = serde_saphyr::options! {};
    let result = serde_saphyr::from_str_with_options::<Value>("a:\n   b: 1\n", options);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn uniform_some_persists_across_documents() {
    let err = parse_multiple(
        RequireIndent::Uniform(Some(2)),
        r#"a:
  b: 1
---
x:
   z: 1
"#,
    )
    .unwrap_err();
    assert_indentation_error(&err, RequireIndent::Uniform(Some(2)), 3);
}

#[test]
fn uniform_some_accepts_matching_second_document() {
    let result = parse_multiple(
        RequireIndent::Uniform(Some(2)),
        r#"a:
  b: 1
---
x:
  z: 1
"#,
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn uniform_none_infers_once_and_stays_consistent_across_documents() {
    let err = parse_multiple(
        RequireIndent::Uniform(None),
        r#"a:
  b: 1
---
x:
   z: 1
"#,
    )
    .unwrap_err();
    assert_indentation_error(&err, RequireIndent::Uniform(Some(2)), 3);
}

fn assert_indentation_error(err: &Error, required: RequireIndent, actual: usize) {
    match err.without_snippet() {
        Error::IndentationError {
            required: got_required,
            actual: got_actual,
            ..
        } => {
            assert_eq!(*got_required, required);
            assert_eq!(*got_actual, actual);
        }
        other => panic!("expected indentation error, got {other:?}"),
    }
}
