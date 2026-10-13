#![cfg(feature = "deserialize")]

//! Regression tests for https://github.com/bourumir-wyngs/serde-saphyr/issues/219.
//! The single-document entry points must report a parser error that follows the root
//! node when the document ended implicitly. Malformed content after an explicit `...`
//! document end marker is still ignored.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_saphyr::Error;

type Map = BTreeMap<String, i32>;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(rename = "a")]
    _a: Option<i32>,
    #[serde(rename = "b")]
    _b: Option<i32>,
}

/// Content after the root node with neither `---` nor `...`; the parser rejects it.
const CONTENT_AFTER_IMPLICIT_END: [&str; 3] =
    ["  a: 1\nb: 2\n", "{a: 1}\n{b: 2}\n", "{a: 1}\nb: 2\n"];

fn single_document_results(input: &str) -> [(&'static str, Result<Map, Error>); 3] {
    [
        ("from_str", serde_saphyr::from_str::<Map>(input)),
        (
            "from_slice",
            serde_saphyr::from_slice::<Map>(input.as_bytes()),
        ),
        (
            "from_reader",
            serde_saphyr::from_reader::<_, Map>(input.as_bytes()),
        ),
    ]
}

fn map_a1() -> Map {
    Map::from([("a".to_owned(), 1)])
}

/// Asserts that `parse` reports the same parser error as `from_str_multiple`, at line 2,
/// column 1, for every input in [`CONTENT_AFTER_IMPLICIT_END`].
fn assert_parser_error_reported<T: std::fmt::Debug>(
    entry_point: &str,
    parse: impl Fn(&str) -> Result<T, Error>,
) {
    for input in CONTENT_AFTER_IMPLICIT_END {
        assert_parser_error_reported_for_input(entry_point, input, &parse);
    }
}

fn assert_parser_error_reported_for_input<T: std::fmt::Debug>(
    entry_point: &str,
    input: &str,
    parse: impl Fn(&str) -> Result<T, Error>,
) {
    let expected = serde_saphyr::from_str_multiple::<Map, Vec<Map>>(input)
        .expect_err("the multi-document entry point reports the parser error");
    let err = match parse(input) {
        Ok(value) => panic!("{entry_point}({input:?}) returned Ok({value:?})"),
        Err(err) => err,
    };
    let err = err.without_snippet();
    assert!(
        !matches!(err, Error::MultipleDocuments { .. }),
        "{entry_point}({input:?}): {err:?}"
    );
    assert_eq!(
        err.to_string(),
        expected.without_snippet().to_string(),
        "{entry_point}({input:?})"
    );
    let location = err.location().expect("parser error location");
    assert_eq!(
        (location.line(), location.column()),
        (2, 1),
        "{entry_point}({input:?})"
    );
}

#[test]
fn from_str_reports_parser_error_after_implicit_document_end() {
    assert_parser_error_reported("from_str", |input| serde_saphyr::from_str::<Map>(input));
}

#[test]
fn from_slice_reports_parser_error_after_implicit_document_end() {
    assert_parser_error_reported("from_slice", |input| {
        serde_saphyr::from_slice::<Map>(input.as_bytes())
    });
}

#[test]
fn from_reader_reports_parser_error_after_implicit_document_end() {
    assert_parser_error_reported("from_reader", |input| {
        serde_saphyr::from_reader::<_, Map>(input.as_bytes())
    });
}

#[test]
fn parser_error_after_implicit_document_end_is_reported_for_structs() {
    assert_parser_error_reported("from_str::<Config>", |input| {
        serde_saphyr::from_str::<Config>(input)
    });
    assert_parser_error_reported("from_reader::<_, Config>", |input| {
        serde_saphyr::from_reader::<_, Config>(input.as_bytes())
    });
}

#[cfg(feature = "validator")]
#[rstest::rstest]
#[case::dedented_mapping("  a: 1\nb: 2\n")]
#[case::consecutive_flow_mappings("{a: 1}\n{b: 2}\n")]
#[case::block_mapping_after_flow_mapping("{a: 1}\nb: 2\n")]
fn validation_entry_point_reports_parser_error_after_implicit_document_end(#[case] input: &str) {
    assert_parser_error_reported_for_input(
        "from_str_validate::<Config>",
        input,
        serde_saphyr::from_str_validate::<Config>,
    );
}

#[test]
fn document_start_marker_still_reports_multiple_documents() {
    let input = "a: 1\n---\nb: 2\n";
    for (entry_point, result) in single_document_results(input) {
        let err = result.expect_err(entry_point);
        assert!(
            matches!(err.without_snippet(), Error::MultipleDocuments { .. }),
            "{entry_point}: {err:?}"
        );
    }
}

#[test]
fn trailing_comment_after_implicit_document_end_is_accepted() {
    let input = "  a: 1\n# trailing comment\n";
    for (entry_point, result) in single_document_results(input) {
        assert_eq!(result.expect(entry_point), map_a1(), "{entry_point}");
    }
}

#[test]
fn malformed_content_after_explicit_document_end_is_ignored() {
    for input in ["a: 1\n...\n@ not yaml\n", "{a: 1}\n...\n`not yaml`\n"] {
        for (entry_point, result) in single_document_results(input) {
            match result {
                Ok(value) => assert_eq!(value, map_a1(), "{entry_point}({input:?})"),
                Err(err) => panic!("{entry_point}({input:?}) failed: {err}"),
            }
        }
    }
}

#[test]
fn document_after_explicit_document_end_reports_multiple_documents() {
    for input in ["a: 1\n...\nb: 2\n", "a: 1\n...\n---\nb: 2\n"] {
        for (entry_point, result) in single_document_results(input) {
            let err = result.expect_err(entry_point);
            assert!(
                matches!(err.without_snippet(), Error::MultipleDocuments { .. }),
                "{entry_point}({input:?}): {err:?}"
            );
        }
    }
}
