#![cfg(feature = "deserialize")]

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_saphyr::{DefaultMessageFormatter, Error, Localizer, Location, SnippetMode};

const SCALAR: &str = "name: &n eighty\nport: *n\n";
const MAPPING: &str = "base: &b\n  port: eighty\ncopy: *b\n";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Port {
    port: u16,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    name: Option<String>,
    port: Option<u16>,
    base: Option<BTreeMap<String, String>>,
    copy: Option<Port>,
}

fn parse(yaml: &str, reader: bool, snippet: bool) -> Error {
    let options = serde_saphyr::options! { with_snippet: snippet };
    if reader {
        serde_saphyr::from_reader_with_options::<_, Config>(yaml.as_bytes(), options).unwrap_err()
    } else {
        serde_saphyr::from_str_with_options::<Config>(yaml, options).unwrap_err()
    }
}

#[test]
fn plain_aliases_distinguish_use_definition_and_failure_locations() {
    for (yaml, expected) in [
        (
            SCALAR,
            "invalid u16 (defined at line 1, column 10) (used at line 2, column 7)",
        ),
        (
            MAPPING,
            "invalid u16 at line 2, column 9 (defined at line 2, column 3) (used at line 3, column 7)",
        ),
    ] {
        for reader in [false, true] {
            let error = parse(yaml, reader, true);
            assert_eq!(error.without_snippet().to_string(), expected);
            assert_eq!(
                error.render_with_options(serde_saphyr::render_options! {
                    snippets: SnippetMode::Off,
                }),
                expected
            );
            let plain = parse(yaml, reader, false);
            assert!(!matches!(plain, Error::WithSnippet { .. }));
            assert_eq!(plain.to_string(), expected);
        }
    }
}

struct Labels;

impl Localizer for Labels {
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
        Cow::Owned(format!("{base} [failure {}:{}]", loc.line(), loc.column()))
    }

    fn alias_defined_at(&self, loc: Location) -> String {
        format!(" [definition {}:{}]", loc.line(), loc.column())
    }

    fn alias_used_at(&self, loc: Location) -> String {
        format!(" [use {}:{}]\n\u{1b}", loc.line(), loc.column())
    }
}

#[test]
fn alias_use_hook_is_independent_and_escaped() {
    let formatter = DefaultMessageFormatter.with_localizer(&Labels);
    assert_eq!(
        parse(MAPPING, false, false).render_with_formatter(&formatter),
        r"invalid u16 [failure 2:9] [definition 2:3] [use 3:7]\n\u{1b}"
    );
    assert_eq!(
        serde_saphyr::from_str::<u16>("eighty")
            .unwrap_err()
            .without_snippet()
            .render_with_formatter(&formatter),
        "invalid u16 [failure 1:1]"
    );
}

#[test]
fn alias_use_remains_explicit_with_missing_or_coincident_definition() {
    for same_location in [false, true] {
        let mut error = parse(SCALAR, false, false);
        let Error::AliasError { locations, .. } = &mut error else {
            panic!("expected an alias error");
        };
        locations.defined_location = if same_location {
            locations.reference_location
        } else {
            Location::UNKNOWN
        };
        assert_eq!(
            error.to_string(),
            "invalid u16 at line 1, column 10 (used at line 2, column 7)"
        );
    }
}

#[test]
fn unknown_alias_use_keeps_the_definition_only_fallback() {
    assert_eq!(
        serde_saphyr::DefaultEnglishLocalizer.alias_used_at(Location::UNKNOWN),
        ""
    );
    let mut error = parse(SCALAR, false, false);
    let Error::AliasError { locations, .. } = &mut error else {
        panic!("expected an alias error");
    };
    locations.reference_location = Location::UNKNOWN;
    assert_eq!(error.to_string(), "invalid u16 at line 1, column 10");
    let formatter = DefaultMessageFormatter.with_localizer(&Labels);
    assert_eq!(
        error.render_with_formatter(&formatter),
        "invalid u16 [failure 1:10]"
    );
}

#[test]
fn snippet_and_miette_do_not_embed_plain_alias_use_suffixes() {
    let error = parse(SCALAR, false, true);
    let formatter = DefaultMessageFormatter.with_localizer(&Labels);
    let rendered = error.render_with_formatter(&formatter);
    assert!(rendered.contains("the value is used here"), "{rendered}");
    assert!(rendered.contains("defined here"), "{rendered}");
    assert!(!rendered.contains("[use"), "{rendered}");
    assert!(!rendered.contains("[definition"), "{rendered}");
    assert!(!rendered.contains("[failure"), "{rendered}");

    #[cfg(feature = "miette")]
    {
        let report = serde_saphyr::miette::to_miette_report_with_formatter(
            &error,
            SCALAR,
            "config.yaml",
            &formatter,
        );
        assert_eq!(report.to_string(), "invalid u16");
        let labels: Vec<_> = report.labels().expect("alias labels").collect();
        assert!(
            labels
                .iter()
                .any(|label| label.label() == Some("the value is used here"))
        );
    }
}
