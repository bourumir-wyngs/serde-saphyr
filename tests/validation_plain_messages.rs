#![cfg(any(feature = "garde", feature = "validator"))]

use serde::Deserialize;
use serde_saphyr::{
    DefaultMessageFormatter, Error, MessageFormatter, Options, SnippetMode, UserMessageFormatter,
};

const YAML: &str = "a: \"\"\nb: \"\"\n";

#[track_caller]
fn assert_two_issue_lines(rendered: &str) {
    let mut lines: Vec<_> = rendered.lines().collect();
    assert_eq!(lines.len(), 2, "expected one line per issue: {rendered:?}");
    // Validator stores fields in a map, so their iteration order is not guaranteed.
    lines.sort_unstable();
    for (line, path, line_number) in [(lines[0], "a", 1), (lines[1], "b", 2)] {
        assert!(
            line.starts_with(&format!("validation error at {path}: ")),
            "missing issue path: {rendered:?}"
        );
        assert!(
            line.ends_with(&format!(" at line {line_number}, column 4")),
            "missing issue location: {rendered:?}"
        );
    }
    assert!(
        !rendered.contains(r"\n"),
        "escaped issue separator: {rendered:?}"
    );
}

#[track_caller]
fn assert_plain_error(error: &Error) {
    assert!(matches!(error, Error::ValidationError { issues, .. } if issues.len() == 2));
    assert_two_issue_lines(&error.to_string());
    assert_two_issue_lines(&error.render_with_formatter(&UserMessageFormatter));
}

#[track_caller]
fn assert_snippets_can_be_disabled(error: &Error) {
    assert!(matches!(error, Error::WithSnippet { .. }));
    for formatter in [
        &DefaultMessageFormatter as &dyn MessageFormatter,
        &UserMessageFormatter,
    ] {
        assert_two_issue_lines(&error.render_with_options(serde_saphyr::render_options! {
            formatter: formatter,
            snippets: SnippetMode::Off,
        }));
    }
}

#[cfg(feature = "garde")]
mod garde {
    use super::*;

    #[derive(Debug, Deserialize, ::garde::Validate)]
    struct Fields {
        #[garde(length(min = 1))]
        a: String,
        #[garde(length(min = 1))]
        b: String,
    }

    #[test]
    fn string_validation_issues_keep_plain_line_separators() {
        let plain = serde_saphyr::from_str_with_options_valid::<Fields>(
            YAML,
            serde_saphyr::options! { with_snippet: false },
        )
        .unwrap_err();
        assert_plain_error(&plain);

        let snippets = serde_saphyr::from_str_valid::<Fields>(YAML).unwrap_err();
        assert_snippets_can_be_disabled(&snippets);
    }

    #[test]
    fn reader_validation_issues_keep_plain_line_separators() {
        let plain = serde_saphyr::from_reader_with_options_valid::<_, Fields>(
            YAML.as_bytes(),
            serde_saphyr::options! { with_snippet: false },
        )
        .unwrap_err();
        assert_plain_error(&plain);

        let snippets = serde_saphyr::from_reader_with_options_valid::<_, Fields>(
            YAML.as_bytes(),
            Options::default(),
        )
        .unwrap_err();
        assert_snippets_can_be_disabled(&snippets);
    }
}

#[cfg(feature = "validator")]
mod validator {
    use super::*;

    #[derive(Debug, Deserialize, ::validator::Validate)]
    struct Fields {
        #[validate(length(min = 1))]
        a: String,
        #[validate(length(min = 1))]
        b: String,
    }

    #[test]
    fn string_validation_issues_keep_plain_line_separators() {
        let plain = serde_saphyr::from_str_with_options_validate::<Fields>(
            YAML,
            serde_saphyr::options! { with_snippet: false },
        )
        .unwrap_err();
        assert_plain_error(&plain);

        let snippets = serde_saphyr::from_str_validate::<Fields>(YAML).unwrap_err();
        assert_snippets_can_be_disabled(&snippets);
    }

    #[test]
    fn reader_validation_issues_keep_plain_line_separators() {
        let plain = serde_saphyr::from_reader_with_options_validate::<_, Fields>(
            YAML.as_bytes(),
            serde_saphyr::options! { with_snippet: false },
        )
        .unwrap_err();
        assert_plain_error(&plain);

        let snippets = serde_saphyr::from_reader_with_options_validate::<_, Fields>(
            YAML.as_bytes(),
            Options::default(),
        )
        .unwrap_err();
        assert_snippets_can_be_disabled(&snippets);
    }
}
