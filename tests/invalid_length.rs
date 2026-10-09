#![cfg(feature = "deserialize")]

use std::borrow::Cow;

use serde::Deserialize;
use serde_saphyr::{Error, Location, MessageFormatter, SnippetMode, UserMessageFormatter};

#[test]
fn serde_invalid_length_retains_the_reported_length_and_expectation() {
    for len in [0, 1, usize::MAX] {
        let error = <Error as serde::de::Error>::invalid_length(len, &"a pair");
        let Error::SerdeInvalidLength {
            len: reported,
            expected,
            location,
        } = &error
        else {
            panic!("expected a structured invalid-length error, got {error:?}");
        };
        assert_eq!(*reported, len);
        assert_eq!(expected, "a pair");
        assert_eq!(*location, Location::UNKNOWN);
        assert_eq!(error.location(), None);
        assert_eq!(error.locations(), None);
        assert_eq!(
            error.to_string(),
            format!("invalid length {len}, expected a pair")
        );
        assert_eq!(
            error.render_with_formatter(&UserMessageFormatter),
            error.to_string()
        );
    }
}

#[test]
fn expectation_text_remains_raw_in_the_error_and_is_escaped_when_rendered() {
    let expectation = "two items\n\u{1b}[31m\u{2028}";
    let error = <Error as serde::de::Error>::invalid_length(1, &expectation);
    let Error::SerdeInvalidLength { expected, .. } = &error else {
        panic!("expected a structured invalid-length error, got {error:?}");
    };
    assert_eq!(expected, expectation);
    let rendered = r"invalid length 1, expected two items\n\u{1b}[31m\u{2028}";
    assert_eq!(error.to_string(), rendered);
    assert_eq!(error.render_with_formatter(&UserMessageFormatter), rendered);
}

struct LengthFormatter;

impl MessageFormatter for LengthFormatter {
    fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        let Error::SerdeInvalidLength { len, expected, .. } = error else {
            panic!("formatter must receive the original invalid-length error, got {error:?}");
        };
        Cow::Owned(format!("length {len}; expected {expected}"))
    }
}

#[test]
fn alias_errors_expose_the_structured_cause_to_callers_and_formatters() {
    #[derive(Debug, Deserialize)]
    struct Config {
        #[serde(rename = "base")]
        _base: Vec<i32>,
        #[serde(rename = "copy")]
        _copy: (i32, i32),
    }

    let yaml = "base: &b [1, 2, 3]\ncopy: *b\n";
    for error in [
        serde_saphyr::from_str::<Config>(yaml).unwrap_err(),
        serde_saphyr::from_reader::<_, Config>(yaml.as_bytes()).unwrap_err(),
    ] {
        assert!(matches!(error, Error::WithSnippet { .. }));
        let alias = error.without_snippet();
        let Error::AliasError {
            error: cause,
            locations,
            ..
        } = alias
        else {
            panic!("expected an alias error, got {alias:?}");
        };
        let Error::SerdeInvalidLength {
            len,
            expected,
            location,
        } = cause.as_ref()
        else {
            panic!("expected a structured invalid-length cause, got {cause:?}");
        };
        assert_eq!(*len, 3);
        assert_eq!(expected, "a tuple of size 2");
        assert_ne!(*location, Location::UNKNOWN);
        assert_eq!(locations.reference_location.line(), 2);
        assert_eq!(locations.reference_location.column(), 7);
        let source = std::error::Error::source(alias)
            .and_then(|source| source.downcast_ref::<Error>())
            .expect("the alias source must expose the original YAML error");
        assert!(std::ptr::eq(source, cause.as_ref()));
        assert!(std::error::Error::source(source).is_none());

        let message = "length 3; expected a tuple of size 2";
        let snippet = error.render_with_formatter(&LengthFormatter);
        assert!(snippet.contains(message), "{snippet}");
        assert!(snippet.contains(" --> "), "{snippet}");
        let plain = error.render_with_options(serde_saphyr::render_options! {
            formatter: &LengthFormatter,
            snippets: SnippetMode::Off,
        });
        assert!(plain.contains(message), "{plain}");
        assert!(!plain.contains(" --> "), "{plain}");

        #[cfg(feature = "miette")]
        {
            let report = serde_saphyr::miette::to_miette_report_with_formatter(
                &error,
                yaml,
                "config.yaml",
                &LengthFormatter,
            );
            assert_eq!(report.to_string(), message);
        }
    }
}
