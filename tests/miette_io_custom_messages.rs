#![cfg(feature = "miette")]

use std::borrow::Cow;

use rstest::rstest;
use serde_saphyr::localizer::DEFAULT_ENGLISH_LOCALIZER;
use serde_saphyr::{
    DefaultMessageFormatter, Error, MessageFormatter, SnippetMode, UserMessageFormatter,
};

struct CustomFormatter {
    include_cause: bool,
}

impl MessageFormatter for CustomFormatter {
    fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        match error {
            Error::IOError { cause } if self.include_cause => {
                Cow::Owned(format!("cannot load configuration: {cause}"))
            }
            _ => Cow::Borrowed("cannot load configuration"),
        }
    }
}

struct StandaloneFormatter;

impl MessageFormatter for StandaloneFormatter {
    fn format_message<'a>(&self, _error: &'a Error) -> Cow<'a, str> {
        Cow::Borrowed("ordinary message")
    }

    fn format_standalone_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        match error {
            Error::IOError { cause } => Cow::Owned(format!("standalone message: {cause}")),
            _ => self.format_message(error),
        }
    }
}

fn assert_standalone_message(formatter: &dyn MessageFormatter, expected: &str) {
    for wrapped in [false, true] {
        let mut error = Error::IOError {
            cause: std::io::Error::other("disk full"),
        };
        if wrapped {
            error = Error::WithSnippet {
                regions: Vec::new(),
                crop_radius: 0,
                error: Box::new(error),
            };
        }
        for snippets in [SnippetMode::Auto, SnippetMode::Off] {
            let report = serde_saphyr::miette::to_miette_report_with_options(
                &error,
                "",
                "config.yaml",
                serde_saphyr::render_options! {
                    formatter: formatter,
                    snippets: snippets,
                },
            );
            assert_eq!(report.to_string(), expected, "{wrapped:?}, {snippets:?}");
            let rendered = format!("{report:?}");
            assert_eq!(rendered.matches(expected).count(), 1, "{rendered}");
            assert_eq!(
                rendered.matches("disk full").count(),
                usize::from(expected.contains("disk full")),
                "{rendered}"
            );
        }
    }
}

#[rstest]
#[case::complete_message(true, "cannot load configuration: disk full")]
#[case::omitted_cause(false, "cannot load configuration")]
fn custom_messages_are_preserved(#[case] include_cause: bool, #[case] expected: &str) {
    assert_standalone_message(&CustomFormatter { include_cause }, expected);
}

#[test]
fn explicit_standalone_hook_is_used() {
    assert_standalone_message(&StandaloneFormatter, "standalone message: disk full");
}

#[test]
fn builtin_formatters_retain_the_cause() {
    let developer = DefaultMessageFormatter.with_localizer(&DEFAULT_ENGLISH_LOCALIZER);
    let user = UserMessageFormatter.with_localizer(&DEFAULT_ENGLISH_LOCALIZER);
    for formatter in [
        &DefaultMessageFormatter as &dyn MessageFormatter,
        &UserMessageFormatter,
        &developer,
        &user,
    ] {
        assert_standalone_message(formatter, "IO error: disk full");
    }
}
