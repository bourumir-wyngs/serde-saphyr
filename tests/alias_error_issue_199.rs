#![cfg(feature = "deserialize")]

//! Regression tests for https://github.com/bourumir-wyngs/serde-saphyr/issues/199.
//! Alias errors must retain the original error and defer diagnostic rendering.

use std::borrow::Cow;
use std::cell::Cell;

use serde::Deserialize;

use serde_saphyr::{
    DefaultMessageFormatter, Error, Localizer, Location, MessageFormatter, from_str,
};

#[derive(Debug, Deserialize)]
struct ScalarConfig {
    #[serde(rename = "name")]
    _name: String,
    #[serde(rename = "port")]
    _port: u16,
}

#[derive(Debug, Deserialize)]
struct Port {
    #[serde(rename = "port")]
    _port: u16,
}

#[derive(Debug, Deserialize)]
struct MappingConfig {
    #[serde(rename = "base")]
    _base: std::collections::BTreeMap<String, String>,
    #[serde(rename = "copy")]
    _copy: Port,
}

fn scalar_alias_error() -> Error {
    from_str::<ScalarConfig>("name: &n eighty\nport: *n\n").unwrap_err()
}

fn mapping_alias_error() -> Error {
    from_str::<MappingConfig>("base: &b\n  port: eighty\ncopy: *b\n").unwrap_err()
}

struct Bracketed;

impl Localizer for Bracketed {
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
        Cow::Owned(format!("{base} [{}:{}]", loc.line(), loc.column()))
    }
}

struct NoLocations;

impl Localizer for NoLocations {
    fn attach_location<'a>(&self, base: Cow<'a, str>, _loc: Location) -> Cow<'a, str> {
        base
    }

    fn alias_defined_at(&self, _loc: Location) -> String {
        String::new()
    }
}

#[derive(Default)]
struct CustomFormatter {
    scalar_calls: Cell<usize>,
}

impl MessageFormatter for CustomFormatter {
    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        match err {
            Error::InvalidScalar { ty: "u16", .. } => {
                self.scalar_calls.set(self.scalar_calls.get() + 1);
                Cow::Borrowed("custom port error")
            }
            _ => DefaultMessageFormatter.format_message(err),
        }
    }
}

#[test]
fn scalar_alias_reports_each_location_once() {
    let outer = scalar_alias_error();
    let error = outer.without_snippet();
    let locations = error.locations().expect("alias locations");

    assert_eq!(
        (
            locations.reference_location.line(),
            locations.reference_location.column()
        ),
        (2, 7)
    );
    assert_eq!(
        (
            locations.defined_location.line(),
            locations.defined_location.column()
        ),
        (1, 10)
    );
    assert_eq!(error.location(), Some(locations.reference_location));
    assert_eq!(
        error.to_string(),
        "invalid u16 (defined at line 1, column 10) at line 2, column 7"
    );
}

#[test]
fn alias_error_preserves_existing_variant_handlers_and_legacy_message() {
    for outer in [scalar_alias_error(), mapping_alias_error()] {
        let error = outer.without_snippet();
        #[allow(deprecated)]
        let legacy_message = match error {
            Error::AliasError { msg, .. } => msg,
            other => panic!("the existing AliasError handler must run, got {other:?}"),
        };
        let Error::AliasError { error: inner, .. } = error else {
            unreachable!("the existing handler matched AliasError")
        };
        assert_eq!(legacy_message, &inner.to_string());
        assert!(legacy_message.contains("invalid u16"), "{legacy_message}");
    }
}

#[test]
fn scalar_alias_respects_custom_location_formatting() {
    let formatter = DefaultMessageFormatter.with_localizer(&Bracketed);
    let direct = from_str::<u16>("eighty").unwrap_err();
    assert_eq!(
        direct.without_snippet().render_with_formatter(&formatter),
        "invalid u16 [1:1]"
    );

    let outer = scalar_alias_error();
    assert_eq!(
        outer.without_snippet().render_with_formatter(&formatter),
        "invalid u16 (defined at line 1, column 10) [2:7]"
    );
}

#[test]
fn scalar_alias_line_offset_adjusts_every_reported_location() {
    struct Offset(u64);

    impl Localizer for Offset {
        fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
            Cow::Owned(format!("{base} [{}:{}]", loc.line() + self.0, loc.column()))
        }

        fn alias_defined_at(&self, loc: Location) -> String {
            format!(" [anchor {}:{}]", loc.line() + self.0, loc.column())
        }
    }

    let formatter = DefaultMessageFormatter.with_localizer(&Offset(100));
    let outer = scalar_alias_error();
    assert_eq!(
        outer.without_snippet().render_with_formatter(&formatter),
        "invalid u16 [anchor 101:10] [102:7]"
    );
}

#[test]
fn aliases_pass_the_original_variant_to_a_custom_formatter() {
    let formatter = CustomFormatter::default();
    let direct = from_str::<u16>("eighty").unwrap_err();
    assert!(matches!(
        direct.without_snippet(),
        Error::InvalidScalar { ty: "u16", .. }
    ));
    assert_eq!(
        direct.without_snippet().render_with_formatter(&formatter),
        "custom port error at line 1, column 1"
    );
    assert_eq!(formatter.scalar_calls.get(), 1);

    for (outer, expected) in [
        (
            scalar_alias_error(),
            "custom port error (defined at line 1, column 10) at line 2, column 7",
        ),
        (
            mapping_alias_error(),
            "custom port error (defined at line 2, column 3) at line 3, column 7",
        ),
    ] {
        formatter.scalar_calls.set(0);
        assert_eq!(
            outer.without_snippet().render_with_formatter(&formatter),
            expected
        );
        assert_eq!(formatter.scalar_calls.get(), 1);

        formatter.scalar_calls.set(0);
        let rendered = outer.render_with_formatter(&formatter);
        assert!(rendered.contains("custom port error"), "{rendered}");
        assert!(!rendered.contains("invalid u16"), "{rendered}");
        assert_eq!(formatter.scalar_calls.get(), 1);
    }
}

#[test]
fn alias_error_source_chain_preserves_the_original_typed_error_and_location() {
    for (outer, expected_location) in [
        (scalar_alias_error(), (1, 10)),
        (mapping_alias_error(), (2, 9)),
    ] {
        let error = outer.without_snippet();
        let snippet_source = std::error::Error::source(&outer)
            .and_then(|source| source.downcast_ref::<Error>())
            .expect("snippet must expose its wrapped error");
        assert!(std::ptr::eq(snippet_source, error));
        let mut source =
            std::error::Error::source(error).expect("alias must expose its inner error");
        while let Some(inner) = source.source() {
            source = inner;
        }
        let original = source.downcast_ref::<Error>().expect("original YAML error");
        assert!(matches!(original, Error::InvalidScalar { ty: "u16", .. }));
        let location = original.location().expect("original scalar location");
        assert_eq!((location.line(), location.column()), expected_location);
    }
}

#[test]
fn mapping_alias_respects_location_suppression_throughout_nested_errors() {
    let formatter = DefaultMessageFormatter.with_localizer(&NoLocations);
    let direct = from_str::<Port>("port: eighty\n").unwrap_err();
    assert_eq!(
        direct.without_snippet().render_with_formatter(&formatter),
        "invalid u16"
    );

    let outer = mapping_alias_error();
    let error = outer.without_snippet();
    let locations = error.locations().expect("alias locations");
    assert_eq!(
        (
            locations.reference_location.line(),
            locations.reference_location.column()
        ),
        (3, 7)
    );
    assert_eq!(
        (
            locations.defined_location.line(),
            locations.defined_location.column()
        ),
        (2, 3)
    );
    assert_eq!(error.render_with_formatter(&formatter), "invalid u16");
}

#[test]
fn mapping_alias_snippet_has_no_embedded_plain_text_locations() {
    let outer = mapping_alias_error();
    assert!(matches!(&outer, Error::WithSnippet { .. }));

    let rendered = outer.render_with_formatter(&DefaultMessageFormatter);
    let title = rendered.lines().next().expect("diagnostic title");
    assert!(title.contains("invalid u16"), "{rendered}");
    assert!(!title.contains(" at line "), "{rendered}");
    assert!(!rendered.contains(" (defined at "), "{rendered}");
    assert!(rendered.contains("the value is used here"), "{rendered}");
    assert!(rendered.contains("defined here"), "{rendered}");
}

#[cfg(feature = "miette")]
#[test]
fn alias_miette_report_uses_custom_message_and_preserves_both_labels() {
    let yaml = "name: &n eighty\nport: *n\n";
    let error = scalar_alias_error();
    let formatter = CustomFormatter::default();
    let report = serde_saphyr::miette::to_miette_report_with_formatter(
        &error,
        yaml,
        "config.yaml",
        &formatter,
    );

    assert_eq!(report.to_string(), "custom port error");
    assert_eq!(formatter.scalar_calls.get(), 1);
    let labels: Vec<_> = report.labels().expect("alias labels").collect();
    assert_eq!(labels.len(), 2);
    assert!(
        labels
            .iter()
            .any(|label| label.label() == Some("the value is used here"))
    );
    assert!(
        labels
            .iter()
            .any(|label| label.label() == Some("anchor defined here"))
    );
}
