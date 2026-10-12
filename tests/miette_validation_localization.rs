#![cfg(all(feature = "miette", any(feature = "garde", feature = "validator")))]

use std::borrow::Cow;
use std::cell::RefCell;

use serde::Deserialize;
use serde_saphyr::{
    DefaultMessageFormatter, Error, ExternalMessage, ExternalMessageSource, Localizer,
    MessageFormatter,
};

#[derive(Debug)]
struct ExternalEntry {
    source: ExternalMessageSource,
    original: String,
    code: Option<String>,
    params: Vec<(String, String)>,
}

#[derive(Default)]
struct Translated {
    entries: RefCell<Vec<ExternalEntry>>,
}

impl Localizer for Translated {
    fn validation_failed(&self, issue_count: usize) -> Cow<'static, str> {
        Cow::Owned(format!("problemes: {issue_count}\n\u{1b}"))
    }

    fn validation_failed_documents(&self, document_count: usize) -> String {
        format!("documents invalides: {document_count}\n\u{1b}")
    }

    fn validation_base_message(&self, entry: &str, resolved_path: &str) -> String {
        format!("champ `{resolved_path}`: {entry}\r")
    }

    fn value_used_here(&self) -> Cow<'static, str> {
        Cow::Owned("utilise ici\n\u{1b}".to_owned())
    }

    fn defined_window(&self) -> Cow<'static, str> {
        Cow::Owned("defini ici\n\u{1b}".to_owned())
    }

    fn override_external_message<'a>(&self, msg: ExternalMessage<'a>) -> Option<Cow<'a, str>> {
        self.entries.borrow_mut().push(ExternalEntry {
            source: msg.source,
            original: msg.original.to_owned(),
            code: msg.code.map(str::to_owned),
            params: msg.params.to_vec(),
        });
        Some(Cow::Owned("trop court\n\u{1b}".to_owned()))
    }
}

#[track_caller]
fn assert_localized_validation(
    error: &Error,
    yaml: &str,
    source: ExternalMessageSource,
    fields: &[&str],
) {
    let Error::ValidationError { issues, .. } = error.without_snippet() else {
        panic!("expected validation issues, got {error:?}");
    };
    assert_eq!(issues.len(), fields.len());

    // Both retained snippet windows and the caller-provided source must use the hooks.
    for error in [error, error.without_snippet()] {
        let localizer = Translated::default();
        let formatter = DefaultMessageFormatter.with_localizer(&localizer);
        let report = serde_saphyr::miette::to_miette_report_with_formatter(
            error,
            yaml,
            "config.yaml",
            &formatter,
        );
        assert_eq!(
            report.to_string(),
            format!("problemes: {}\\n\\u{{1b}}", fields.len())
        );
        let related: Vec<_> = report.related().expect("validation diagnostics").collect();
        assert_eq!(related.len(), fields.len());
        let mut messages: Vec<_> = related.iter().map(|entry| entry.to_string()).collect();
        messages.sort_unstable();
        let mut expected: Vec<_> = fields
            .iter()
            .map(|field| format!("champ `{field}`: trop court\\n\\u{{1b}}\\r"))
            .collect();
        expected.sort_unstable();
        assert_eq!(messages, expected);
        for entry in related {
            let labels: Vec<_> = entry.labels().expect("validation labels").collect();
            assert!(!labels.is_empty());
            assert_eq!(labels[0].label(), Some(r"utilise ici\n\u{1b}"));
            for definition in labels.iter().skip(1) {
                assert_eq!(definition.label(), Some(r"defini ici\n\u{1b}"));
            }
        }

        let entries = localizer.entries.borrow();
        assert_eq!(entries.len(), issues.len());
        for (entry, issue) in entries.iter().zip(issues) {
            assert_eq!(entry.source, source);
            assert_eq!(entry.code.as_deref(), Some(issue.code.as_str()));
            assert_eq!(entry.params, issue.params);
            if let Some(message) = &issue.message {
                assert_eq!(&entry.original, message);
            } else {
                assert!(entry.original.starts_with(&issue.code), "{entry:?}");
            }
            match source {
                ExternalMessageSource::Garde => {
                    assert_eq!(entry.code.as_deref(), Some("garde"));
                    assert!(entry.params.is_empty());
                }
                ExternalMessageSource::Validator => {
                    assert_eq!(entry.code.as_deref(), Some("length"));
                    assert!(
                        entry
                            .params
                            .iter()
                            .any(|(key, value)| key == "min" && value == "3")
                    );
                    assert!(entry.params.iter().any(|(key, _)| key == "value"));
                }
                _ => panic!("unexpected validation source"),
            }
        }
    }
}

struct AggregateFormatter<'a>(&'a Translated);

impl MessageFormatter for AggregateFormatter<'_> {
    fn localizer(&self) -> &dyn Localizer {
        self.0
    }

    fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        if let Error::ValidationErrors { errors, .. } = error {
            Cow::Owned(format!("custom aggregate: {}\n\u{1b}", errors.len()))
        } else {
            DefaultMessageFormatter
                .with_localizer(self.0)
                .format_message(error)
        }
    }
}

#[track_caller]
fn assert_localized_aggregate(error: &Error, yaml: &str) {
    let Error::ValidationErrors { errors, .. } = error else {
        panic!("expected multiple document errors, got {error:?}");
    };
    assert_eq!(errors.len(), 2);
    let localizer = Translated::default();
    let formatter = DefaultMessageFormatter.with_localizer(&localizer);
    let report = serde_saphyr::miette::to_miette_report_with_formatter(
        error,
        yaml,
        "config.yaml",
        &formatter,
    );
    assert_eq!(report.to_string(), r"documents invalides: 2\n\u{1b}");
    let related: Vec<_> = report.related().expect("document diagnostics").collect();
    assert_eq!(related.len(), 2);
    for document in related {
        assert_eq!(document.to_string(), r"problemes: 1\n\u{1b}");
        assert_eq!(
            document
                .related()
                .expect("validation issue")
                .next()
                .unwrap()
                .to_string(),
            r"champ `a`: trop court\n\u{1b}\r"
        );
    }

    let report = serde_saphyr::miette::to_miette_report_with_formatter(
        error,
        yaml,
        "config.yaml",
        &AggregateFormatter(&localizer),
    );
    assert_eq!(report.to_string(), r"custom aggregate: 2\n\u{1b}");
    assert_eq!(report.related().expect("document diagnostics").count(), 2);
}

#[cfg(feature = "garde")]
mod garde {
    use super::*;

    #[derive(Debug, Deserialize, ::garde::Validate)]
    struct Fields {
        #[garde(length(min = 3))]
        a: String,
        #[garde(length(min = 3))]
        b: String,
    }

    #[test]
    fn actual_validation_messages_are_localized_for_string_and_reader_input() {
        for (yaml, fields) in [
            ("a: no\nb: yes\n", &["a"][..]),
            ("a: &word no\nb: *word\n", &["a", "b"][..]),
        ] {
            for error in [
                serde_saphyr::from_str_valid::<Fields>(yaml).unwrap_err(),
                serde_saphyr::from_reader_valid::<_, Fields>(yaml.as_bytes()).unwrap_err(),
            ] {
                assert_localized_validation(&error, yaml, ExternalMessageSource::Garde, fields);
            }
        }
    }

    #[test]
    fn multiple_document_summary_uses_localizer_and_custom_formatter() {
        let yaml = "a: no\nb: yes\n---\na: no\nb: yes\n";
        let error = serde_saphyr::from_multiple_valid::<Fields>(yaml).unwrap_err();
        assert_localized_aggregate(&error, yaml);
    }
}

#[cfg(feature = "validator")]
mod validator {
    use super::*;

    #[derive(Debug, Deserialize, ::validator::Validate)]
    struct Fields {
        #[validate(length(min = 3))]
        a: String,
        #[validate(length(min = 3))]
        b: String,
    }

    #[test]
    fn actual_validation_messages_are_localized_for_string_and_reader_input() {
        for (yaml, fields) in [
            ("a: no\nb: yes\n", &["a"][..]),
            ("a: &word no\nb: *word\n", &["a", "b"][..]),
        ] {
            for error in [
                serde_saphyr::from_str_validate::<Fields>(yaml).unwrap_err(),
                serde_saphyr::from_reader_validate::<_, Fields>(yaml.as_bytes()).unwrap_err(),
            ] {
                assert_localized_validation(&error, yaml, ExternalMessageSource::Validator, fields);
            }
        }
    }

    #[test]
    fn multiple_document_summary_uses_localizer_and_custom_formatter() {
        let yaml = "a: no\nb: yes\n---\na: no\nb: yes\n";
        let error = serde_saphyr::from_multiple_validate::<Fields>(yaml).unwrap_err();
        assert_localized_aggregate(&error, yaml);
    }
}
