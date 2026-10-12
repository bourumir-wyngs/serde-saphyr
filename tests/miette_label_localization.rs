#![cfg(feature = "miette")]

use std::borrow::Cow;
use std::collections::BTreeMap;

use miette::Diagnostic;
use serde::Deserialize;
use serde_saphyr::{DefaultMessageFormatter, Error, Localizer, Location, MessageFormatter};

struct Localized;

impl Localizer for Localized {
    fn value_used_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("UTILISÉ\n\u{1b}")
    }

    fn anchor_defined_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("ANCRE\n\u{1b}")
    }

    fn defined_window(&self) -> Cow<'static, str> {
        Cow::Borrowed("DÉFINITION\n\u{1b}")
    }

    fn error_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("ERREUR\n\u{1b}")
    }

    fn included_from_here(&self) -> Cow<'static, str> {
        Cow::Borrowed("INCLUS\n\u{1b}")
    }
}

#[derive(Debug, Deserialize)]
struct Port {
    #[serde(rename = "port")]
    _port: u16,
}

#[derive(Debug, Deserialize)]
struct MappingConfig {
    #[serde(rename = "base")]
    _base: BTreeMap<String, String>,
    #[serde(rename = "copy")]
    _copy: Port,
}

fn label_texts(diagnostic: &dyn Diagnostic) -> Vec<String> {
    let mut labels: Vec<_> = diagnostic
        .labels()
        .into_iter()
        .flatten()
        .filter_map(|label| label.label().map(str::to_owned))
        .collect();
    for related in diagnostic.related().into_iter().flatten() {
        labels.extend(label_texts(related));
    }
    labels.sort();
    labels
}

#[test]
fn alias_labels_and_related_titles_use_escaped_localizer_text() {
    let mut distant_yaml = String::from("base: &b\n");
    for index in 0..30 {
        distant_yaml.push_str(&format!("  before_{index}: valid\n"));
    }
    distant_yaml.push_str("  port: eighty\n");
    for index in 0..30 {
        distant_yaml.push_str(&format!("  after_{index}: valid\n"));
    }
    distant_yaml.push_str("copy: *b\n");

    let localized = DefaultMessageFormatter.with_localizer(&Localized);
    for yaml in ["base: &b\n  port: eighty\ncopy: *b\n", &distant_yaml] {
        for error in [
            serde_saphyr::from_str::<MappingConfig>(yaml).unwrap_err(),
            serde_saphyr::from_reader::<_, MappingConfig>(yaml.as_bytes()).unwrap_err(),
        ] {
            for with_snippet in [false, true] {
                let error = if with_snippet {
                    &error
                } else {
                    error.without_snippet()
                };
                for (formatter, expected) in [
                    (
                        &DefaultMessageFormatter as &dyn MessageFormatter,
                        [
                            "the value is used here",
                            "anchor defined here",
                            "the error occurred here",
                        ],
                    ),
                    (
                        &localized,
                        [r"UTILISÉ\n\u{1b}", r"ANCRE\n\u{1b}", r"ERREUR\n\u{1b}"],
                    ),
                ] {
                    let report = serde_saphyr::miette::to_miette_report_with_formatter(
                        error,
                        yaml,
                        "config.yaml",
                        formatter,
                    );
                    let mut expected_labels = expected.map(str::to_owned).to_vec();
                    expected_labels.sort();
                    assert_eq!(label_texts(report.as_ref()), expected_labels);

                    if with_snippet && yaml == distant_yaml {
                        let related: Vec<_> = report.related().unwrap().collect();
                        assert_eq!(related.len(), 2);
                        assert!(related.iter().any(|entry| entry.to_string() == expected[1]));
                    }
                }
            }
        }
    }
}

#[test]
fn definition_only_alias_uses_the_localized_definition_window_label() {
    let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
    let error = serde_saphyr::from_str::<MappingConfig>(yaml).unwrap_err();
    let mut error = match error {
        Error::WithSnippet { error, .. } => *error,
        other => other,
    };
    let Error::AliasError { locations, .. } = &mut error else {
        panic!("expected an alias error");
    };
    locations.reference_location = Location::UNKNOWN;

    let localized = DefaultMessageFormatter.with_localizer(&Localized);
    for (formatter, expected) in [
        (
            &DefaultMessageFormatter as &dyn MessageFormatter,
            ["defined here", "the error occurred here"],
        ),
        (&localized, [r"DÉFINITION\n\u{1b}", r"ERREUR\n\u{1b}"]),
    ] {
        let report = serde_saphyr::miette::to_miette_report_with_formatter(
            &error,
            yaml,
            "config.yaml",
            formatter,
        );
        let mut expected_labels = expected.map(str::to_owned).to_vec();
        expected_labels.sort();
        assert_eq!(label_texts(report.as_ref()), expected_labels);
    }
}

#[cfg(feature = "include")]
#[test]
fn include_related_title_uses_escaped_localizer_text() {
    let yaml = "port: !include port.yaml\n";
    let options =
        serde_saphyr::options! {}.with_include_resolver(|request: serde_saphyr::IncludeRequest| {
            assert_eq!(request.spec, "port.yaml");
            Ok(serde_saphyr::ResolvedInclude::new(
                "port.yaml",
                "port.yaml",
                serde_saphyr::InputSource::from_string("eighty\n".to_owned()),
            ))
        });
    let error = serde_saphyr::from_str_with_options::<Port>(yaml, options).unwrap_err();
    let localized = DefaultMessageFormatter.with_localizer(&Localized);
    for (formatter, expected) in [
        (
            &DefaultMessageFormatter as &dyn MessageFormatter,
            "included from here",
        ),
        (&localized, r"INCLUS\n\u{1b}"),
    ] {
        let report = serde_saphyr::miette::to_miette_report_with_formatter(
            &error,
            yaml,
            "config.yaml",
            formatter,
        );
        let related: Vec<_> = report.related().expect("include-site diagnostic").collect();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].to_string(), expected);
        assert_eq!(related[0].labels().unwrap().count(), 1);
        let rendered = error.render_with_formatter(formatter);
        assert!(rendered.contains(&format!("{expected}:")), "{rendered}");
    }
}
