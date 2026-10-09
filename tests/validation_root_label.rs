#![cfg(feature = "garde")]

use std::borrow::Cow;

use serde::Deserialize;
use serde_saphyr::{DefaultMessageFormatter, Localizer, SnippetMode, UserMessageFormatter};

#[derive(Debug, Deserialize, garde::Validate)]
#[garde(transparent)]
struct Root(#[garde(length(min = 3))] String);

struct RootLabel(&'static str);

impl Localizer for RootLabel {
    fn root_path_label(&self) -> Cow<'static, str> {
        Cow::Borrowed(self.0)
    }
}

#[test]
fn root_validation_uses_the_localized_label() {
    for error in [
        serde_saphyr::from_str_valid::<Root>("ab").unwrap_err(),
        serde_saphyr::from_reader_valid::<_, Root>("ab".as_bytes()).unwrap_err(),
    ] {
        assert_eq!(
            error.to_string(),
            "validation error at <root>: length is lower than 3"
        );
        for (label, escaped) in [
            ("ROOT", "ROOT"),
            ("", ""),
            ("ROOT\n\u{1b}", r"ROOT\n\u{1b}"),
        ] {
            let localizer = RootLabel(label);
            let expected = format!("validation error at {escaped}: length is lower than 3");
            let formatter = DefaultMessageFormatter.with_localizer(&localizer);
            assert_eq!(error.render_with_formatter(&formatter), expected);
            assert_eq!(
                error.render_with_options(serde_saphyr::render_options! {
                    formatter: &formatter,
                    snippets: SnippetMode::Off,
                }),
                expected
            );
            assert_eq!(
                error.render_with_formatter(&UserMessageFormatter.with_localizer(&localizer)),
                expected
            );
        }
    }
}

#[cfg(feature = "miette")]
#[test]
fn miette_root_validation_uses_the_localized_label() {
    let error = serde_saphyr::from_str_valid::<Root>("ab").unwrap_err();
    for (label, escaped) in [
        ("ROOT", "ROOT"),
        ("", ""),
        ("ROOT\n\u{1b}", r"ROOT\n\u{1b}"),
    ] {
        let localizer = RootLabel(label);
        let formatter = DefaultMessageFormatter.with_localizer(&localizer);
        let report = serde_saphyr::miette::to_miette_report_with_formatter(
            &error,
            "ab",
            "config.yaml",
            &formatter,
        );
        let mut related = report.related().expect("validation issue diagnostics");
        assert_eq!(
            related.next().expect("root validation issue").to_string(),
            format!("validation error: length is lower than 3 for `{escaped}`")
        );
        assert!(related.next().is_none());
    }
}

#[test]
fn validation_snippets_use_the_localized_root_label() {
    #[derive(Debug, Deserialize, garde::Validate)]
    struct Fields {
        #[garde(length(min = 3))]
        value: String,
    }

    let mut error = serde_saphyr::from_str_valid::<Fields>("value: ab\n").unwrap_err();
    let serde_saphyr::Error::WithSnippet {
        error: inner,
        regions,
        ..
    } = &mut error
    else {
        panic!("expected a snippet wrapper");
    };
    assert!(!regions.is_empty(), "the field error must have a snippet");
    let serde_saphyr::Error::ValidationError { issues, .. } = inner.as_mut() else {
        panic!("expected validation issues");
    };
    // Model a validation report containing both a field and a document-level failure.
    issues.push(
        serde_saphyr::ValidationIssue::new(serde_saphyr::path_map::PathKey::new(), "root")
            .with_message("invalid document"),
    );

    let formatter = DefaultMessageFormatter.with_localizer(&RootLabel("ROOT"));
    let rendered = error.render_with_formatter(&formatter);
    assert!(rendered.contains(" --> "), "expected a snippet: {rendered}");
    assert!(rendered.contains("for `value`"), "{rendered}");
    assert!(
        rendered.contains("validation error: invalid document for `ROOT`"),
        "{rendered}"
    );
    assert!(!rendered.contains("<root>"), "{rendered}");
}
