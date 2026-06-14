#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde_json::Value;
use serde_saphyr::RequireIndent;

fn parse(require: RequireIndent, yaml: &str) -> Result<Value, String> {
    let options = serde_saphyr::options! { require_indent: require };
    serde_saphyr::from_str_with_options::<Value>(yaml, options).map_err(|e| e.to_string())
}

fn parse_multiple(require: RequireIndent, yaml: &str) -> Result<Vec<Value>, String> {
    let options = serde_saphyr::options! { require_indent: require };
    serde_saphyr::from_multiple_with_options::<Value>(yaml, options).map_err(|e| e.to_string())
}

#[rstest]
#[case::even_two(RequireIndent::Even, "root:\n  child: value\n")]
#[case::divisible_four(RequireIndent::Divisible(4), "root:\n    child: value\n")]
#[case::uniform_inferred(RequireIndent::Uniform(None), "a:\n  b: 1\n  c: 2\n")]
#[case::uniform_two(RequireIndent::Uniform(Some(2)), "x:\n  y:\n    z: 1\n")]
#[case::unchecked(RequireIndent::Unchecked, "a:\n   b:\n       c: 1\n")]
#[case::block_scalar_unchecked(RequireIndent::Uniform(Some(2)), "x: |\n  foo:\ny: |\n   bar\n")]
fn accepts_valid_indentation(#[case] require: RequireIndent, #[case] yaml: &str) {
    assert!(parse(require, yaml).is_ok());
}

#[rstest]
#[case::even_rejects_odd(RequireIndent::Even, "root:\n   child: value\n")]
#[case::divisible_four_rejects_two(RequireIndent::Divisible(4), "root:\n  child: value\n")]
#[case::uniform_rejects_mixed(RequireIndent::Uniform(None), "a:\n  b:\n     c: 1\n")]
fn rejects_invalid_indentation(#[case] require: RequireIndent, #[case] yaml: &str) {
    let err = parse(require, yaml).unwrap_err();
    assert!(err.contains("indentation"), "{err}");
}

// Regression for https://github.com/bourumir-wyngs/serde-saphyr/issues/132.
#[rstest]
#[case::first_indent_too_wide("x:\n   z: 1\n", 3)]
#[case::first_indent_too_narrow("x:\n z: 1\n", 1)]
#[case::inferred_then_off_multiple("x:\n y:\n   z: 1\n", 1)]
#[case::valid_first_then_off("x:\n  y:\n   z: 1\n", 3)]
fn uniform_some_reports_configured_value(#[case] yaml: &str, #[case] found: usize) {
    let err = parse(RequireIndent::Uniform(Some(2)), yaml).unwrap_err();
    assert!(
        err.contains(&format!(
            "expected uniform (2 spaces), found {found} spaces"
        )),
        "{err}"
    );
}

#[test]
fn default_is_unchecked() {
    let options = serde_saphyr::options! {};
    let result = serde_saphyr::from_str_with_options::<Value>("a:\n   b: 1\n", options);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn uniform_some_persists_across_documents() {
    let err = parse_multiple(RequireIndent::Uniform(Some(2)), r#"a:
  b: 1
---
x:
   z: 1
"#)
        .unwrap_err();
    assert!(
        err.contains("expected uniform (2 spaces), found 3 spaces"),
        "{err}"
    );
}

#[test]
fn uniform_some_accepts_matching_second_document() {
    let result = parse_multiple(RequireIndent::Uniform(Some(2)), r#"a:
  b: 1
---
x:
  z: 1
"#);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn uniform_none_reinfers_per_document() {
    let result = parse_multiple(RequireIndent::Uniform(None), r#"a:
  b: 1
---
x:
    z: 1
"#);
    assert!(result.is_ok(), "{result:?}");
}
