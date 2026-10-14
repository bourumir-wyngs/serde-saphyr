#![cfg(feature = "deserialize")]

//! Regression tests for https://github.com/bourumir-wyngs/serde-saphyr/issues/228.
//! An explicit `...` ends a document, not the stream. Whitespace and comments
//! after it are accepted. Single-document entry points report parser errors
//! encountered by their look-ahead, or `MultipleDocuments` if the look-ahead
//! reaches a following document's first content event.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_saphyr::{Error, Options};

type Map = BTreeMap<String, i32>;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "garde", derive(garde::Validate))]
#[cfg_attr(feature = "validator", derive(validator::Validate))]
struct Config {
    #[cfg_attr(feature = "garde", garde(skip))]
    a: i32,
}

impl From<Config> for Map {
    fn from(config: Config) -> Self {
        Map::from([("a".to_owned(), config.a)])
    }
}

/// Single-document entry points covering the shared trailing-content check,
/// each reading `input` into a [`Map`].
fn single_document_results(input: &str) -> Vec<(&'static str, Result<Map, Error>)> {
    #[allow(unused_mut)]
    let mut results = vec![
        ("from_str", serde_saphyr::from_str::<Map>(input)),
        (
            "from_str_with_options",
            serde_saphyr::from_str_with_options::<Map>(input, Options::default()),
        ),
        (
            "from_slice",
            serde_saphyr::from_slice::<Map>(input.as_bytes()),
        ),
        (
            "from_slice_with_options",
            serde_saphyr::from_slice_with_options::<Map>(input.as_bytes(), Options::default()),
        ),
        (
            "from_reader",
            serde_saphyr::from_reader::<_, Map>(input.as_bytes()),
        ),
        (
            "from_reader_with_options",
            serde_saphyr::from_reader_with_options::<_, Map>(input.as_bytes(), Options::default()),
        ),
        (
            "with_deserializer_from_str",
            serde_saphyr::with_deserializer_from_str(input, |de| Map::deserialize(de)),
        ),
        (
            "with_deserializer_from_reader",
            serde_saphyr::with_deserializer_from_reader(input.as_bytes(), |de| {
                Map::deserialize(de)
            }),
        ),
    ];
    #[cfg(feature = "garde")]
    results.extend([
        (
            "from_str_valid",
            serde_saphyr::from_str_valid::<Config>(input).map(Map::from),
        ),
        (
            "from_reader_valid",
            serde_saphyr::from_reader_valid::<_, Config>(input.as_bytes()).map(Map::from),
        ),
    ]);
    #[cfg(feature = "validator")]
    results.extend([
        (
            "from_str_validate",
            serde_saphyr::from_str_validate::<Config>(input).map(Map::from),
        ),
        (
            "from_reader_validate",
            serde_saphyr::from_reader_validate::<_, Config>(input.as_bytes()).map(Map::from),
        ),
    ]);
    results
}

fn map_a1() -> Map {
    Map::from([("a".to_owned(), 1)])
}

#[rstest::rstest]
#[case::bad_directive("a: 1\n...\n%YAML nope\n")]
#[case::bad_directive_before_document("a: 1\n...\n%YAML nope\n---\nb: 2\n")]
#[case::unclosed_flow_sequence("a: 1\n...\n[unclosed\n")]
#[case::stray_flow_sequence_end("a: 1\n...\n]\n")]
#[case::stray_flow_mapping_end("a: 1\n...\n}\n")]
#[case::unterminated_quote("a: 1\n...\n\"unterminated\n")]
#[case::reserved_indicator("a: 1\n...\n@ not yaml\n")]
#[case::reserved_indicator_after_flow_root("{a: 1}\n...\n`not yaml`\n")]
#[case::marker_with_comment("a: 1\n... # end\n%YAML nope\n")]
#[case::explicit_document_start("---\na: 1\n...\n[unclosed\n")]
#[case::repeated_marker("a: 1\n...\n...\n[unclosed\n")]
#[case::crlf("a: 1\r\n...\r\n[unclosed\r\n")]
fn malformed_content_after_explicit_document_end_is_reported(#[case] input: &str) {
    let expected = serde_saphyr::from_str_multiple::<Map, Vec<Map>>(input)
        .expect_err("the multi-document entry point reports the parser error");
    let expected = expected.without_snippet();
    for (entry_point, result) in single_document_results(input) {
        let err = match result {
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
            expected.to_string(),
            "{entry_point}({input:?})"
        );
        assert_eq!(
            err.location(),
            expected.location(),
            "{entry_point}({input:?})"
        );
    }
}

#[rstest::rstest]
#[case::document_start("a: 1\n...\n---\nb: 2\n")]
#[case::bare_document("a: 1\n...\nb: 2\n")]
#[case::bare_flow_document("a: 1\n...\n{b: 2}\n")]
#[case::directive_then_document_start("a: 1\n...\n%YAML 1.2\n---\nb: 2\n")]
#[case::empty_document("a: 1\n...\n---\n")]
fn document_after_explicit_document_end_reports_multiple_documents(#[case] input: &str) {
    serde_saphyr::from_str_multiple::<Map, Vec<Map>>(input)
        .expect("the multi-document entry point accepts the stream");
    for (entry_point, result) in single_document_results(input) {
        let err = result.expect_err(entry_point);
        assert!(
            matches!(err.without_snippet(), Error::MultipleDocuments { .. }),
            "{entry_point}({input:?}): {err:?}"
        );
    }
}

#[rstest::rstest]
#[case::marker_at_end_of_input("a: 1\n...")]
#[case::newline("a: 1\n...\n")]
#[case::blank_lines("a: 1\n...\n\n   \n\n")]
#[case::comment("a: 1\n...\n# comment\n")]
#[case::comment_on_marker_line("a: 1\n... # end\n")]
#[case::repeated_marker("a: 1\n...\n...\n")]
#[case::explicit_document_start("---\na: 1\n...\n# comment\n")]
#[case::crlf("a: 1\r\n...\r\n# comment\r\n")]
fn whitespace_and_comments_after_explicit_document_end_are_accepted(#[case] input: &str) {
    for (entry_point, result) in single_document_results(input) {
        match result {
            Ok(value) => assert_eq!(value, map_a1(), "{entry_point}({input:?})"),
            Err(err) => panic!("{entry_point}({input:?}) failed: {err}"),
        }
    }
}
