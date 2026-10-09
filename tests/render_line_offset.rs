#![cfg(feature = "deserialize")]

use std::borrow::Cow;

use serde::Deserialize;
use serde_saphyr::{
    DefaultMessageFormatter, Error, Localizer, Location, MessageFormatter, SnippetMode,
};

const LINE_OFFSET: u64 = 9_999;

fn render(error: &Error, snippets: SnippetMode) -> String {
    error.render_with_options(serde_saphyr::render_options! {
        line_offset: LINE_OFFSET,
        snippets: snippets,
    })
}

#[track_caller]
fn assert_marker_points_to(rendered: &str, source: &str, value: &str) {
    let mut lines = rendered.lines();
    let source_line = lines
        .find(|line| line.contains(source))
        .expect("source line");
    let marker_line = lines.next().expect("marker line");
    assert_eq!(marker_line.find('^'), source_line.find(value), "{rendered}");
}

#[test]
fn string_and_reader_render_first_line_as_10000_without_changing_stored_locations() {
    for error in [
        serde_saphyr::from_str::<u8>("eighty\n").unwrap_err(),
        serde_saphyr::from_reader::<_, u8>("eighty\n".as_bytes()).unwrap_err(),
    ] {
        let original_location = error.location().expect("scalar location");
        let original_span = original_location.span();
        let original_render = error.to_string();
        assert_eq!(original_location.line(), 1);
        assert_eq!(
            render(&error, SnippetMode::Off),
            "invalid u8 at line 10000, column 1"
        );

        let rendered = render(&error, SnippetMode::Auto);
        assert!(rendered.contains(":10000:1"), "{rendered}");
        assert!(rendered.contains("10000 | eighty"), "{rendered}");
        assert_marker_points_to(&rendered, "eighty", "eighty");
        assert_eq!(render(&error, SnippetMode::Auto), rendered);
        assert_eq!(error.location(), Some(original_location));
        assert_eq!(error.location().unwrap().span(), original_span);
        assert_eq!(error.to_string(), original_render);
        assert!(original_render.contains(":1:1"), "{original_render}");
    }
}

#[test]
fn reader_snapshot_keeps_the_correct_lines_when_its_window_starts_late() {
    let mut yaml = String::new();
    for _ in 0..40 {
        yaml.push_str("# preceding context\n");
    }
    yaml.push_str("eighty\n");
    let error = serde_saphyr::from_reader::<_, u8>(yaml.as_bytes()).unwrap_err();
    let rendered = render(&error, SnippetMode::Auto);
    assert!(rendered.contains(":10040:1"), "{rendered}");
    assert!(rendered.contains("10040 | eighty"), "{rendered}");
    assert_marker_points_to(&rendered, "eighty", "eighty");
}

#[derive(Debug, Deserialize)]
struct Port {
    #[serde(rename = "port")]
    _port: u16,
}

#[derive(Debug, Deserialize)]
struct AliasedPort {
    #[serde(rename = "base")]
    _base: std::collections::BTreeMap<String, String>,
    #[serde(rename = "copy")]
    _copy: Port,
}

#[test]
fn alias_use_definition_and_failing_value_all_receive_the_offset() {
    let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
    for error in [
        serde_saphyr::from_str::<AliasedPort>(yaml).unwrap_err(),
        serde_saphyr::from_reader::<_, AliasedPort>(yaml.as_bytes()).unwrap_err(),
    ] {
        assert_eq!(
            render(&error, SnippetMode::Off),
            "invalid u16 at line 10001, column 9 (defined at line 10001, column 3) (used at line 10002, column 7)"
        );
        let rendered = render(&error, SnippetMode::Auto);
        for location in [":10001:9", "anchor at line 10001 column 3", ":10002:7"] {
            assert!(
                rendered.contains(location),
                "missing {location}: {rendered}"
            );
        }
        assert!(rendered.contains("10000 | base: &b"), "{rendered}");
        assert!(rendered.contains("10001 |   port: eighty"), "{rendered}");
    }
}

struct Bracketed;

impl Localizer for Bracketed {
    fn attach_location<'a>(&self, base: Cow<'a, str>, loc: Location) -> Cow<'a, str> {
        Cow::Owned(format!("{base} [{}:{}]", loc.line(), loc.column()))
    }

    fn alias_used_at(&self, loc: Location) -> String {
        format!(" [used {}:{}]", loc.line(), loc.column())
    }

    fn alias_defined_at(&self, loc: Location) -> String {
        format!(" [defined {}:{}]", loc.line(), loc.column())
    }

    #[cfg(any(feature = "garde", feature = "validator"))]
    fn validation_issue_line(&self, path: &str, _entry: &str, loc: Option<Location>) -> String {
        let loc = loc.expect("field location");
        format!("check {path} [{}:{}]", loc.line(), loc.column())
    }
}

#[test]
fn custom_localizer_receives_shifted_alias_locations() {
    let error =
        serde_saphyr::from_str::<AliasedPort>("base: &b\n  port: eighty\ncopy: *b\n").unwrap_err();
    let formatter = DefaultMessageFormatter.with_localizer(&Bracketed);
    let rendered = error.render_with_options(serde_saphyr::render_options! {
        formatter: &formatter,
        line_offset: LINE_OFFSET,
        snippets: SnippetMode::Off,
    });
    assert_eq!(
        rendered,
        "invalid u16 [10001:9] [defined 10001:3] [used 10002:7]"
    );
}

struct CustomMessage;

impl MessageFormatter for CustomMessage {
    fn format_message<'a>(&self, _error: &'a Error) -> Cow<'a, str> {
        Cow::Borrowed("custom error")
    }
}

#[test]
fn custom_message_is_preserved_and_unknown_locations_stay_unknown() {
    let error = serde_saphyr::from_str::<u8>("eighty").unwrap_err();
    let rendered = error.render_with_options(serde_saphyr::render_options! {
        formatter: &CustomMessage,
        line_offset: LINE_OFFSET,
        snippets: SnippetMode::Off,
    });
    assert_eq!(rendered, "custom error at line 10000, column 1");

    let error = Error::UnknownAnchor {
        location: Location::UNKNOWN,
    };
    for snippets in [SnippetMode::Off, SnippetMode::Auto] {
        assert_eq!(render(&error, snippets), error.to_string());
    }
}

#[cfg(feature = "include")]
#[test]
fn only_root_source_locations_and_include_sites_are_shifted() {
    use serde_saphyr::{IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude};

    let make_options = || {
        serde_saphyr::options! {}.with_include_resolver(
            |request: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
                assert_eq!(request.spec, "port.yaml");
                Ok(ResolvedInclude::new(
                    "port.yaml",
                    "port.yaml",
                    InputSource::from_string("eighty\n".to_owned()),
                ))
            },
        )
    };
    let yaml = "port: !include port.yaml\n";
    for error in [
        serde_saphyr::from_str_with_options::<Port>(yaml, make_options()).unwrap_err(),
        serde_saphyr::from_reader_with_options::<_, Port>(yaml.as_bytes(), make_options())
            .unwrap_err(),
    ] {
        assert_eq!(
            render(&error, SnippetMode::Off),
            "invalid u16 at line 1, column 1"
        );
        let rendered = render(&error, SnippetMode::Auto);
        assert!(rendered.contains("port.yaml:1:1"), "{rendered}");
        assert!(rendered.contains("1 | eighty"), "{rendered}");
        assert!(
            rendered.contains("10000 | port: !include port.yaml"),
            "{rendered}"
        );
        assert!(!rendered.contains("port.yaml:10000:"), "{rendered}");
    }

    // Enabling includes also assigns a source id to the root document.
    let error =
        serde_saphyr::from_str_with_options::<Port>("port: eighty\n", make_options()).unwrap_err();
    assert_eq!(
        render(&error, SnippetMode::Off),
        "invalid u16 at line 10000, column 7"
    );
}

#[cfg(any(feature = "garde", feature = "validator"))]
mod validation {
    use super::*;

    const YAML: &str = "a: \"\"\nb: \"\"\n";

    #[track_caller]
    fn assert_validation(error: &Error) {
        let original = error.to_string();
        let plain = render(error, SnippetMode::Off);
        assert_eq!(plain.lines().count(), 2, "{plain}");
        for (field, line) in [("a", 10000), ("b", 10001)] {
            let issue = plain
                .lines()
                .find(|text| text.contains(&format!("at {field}:")))
                .unwrap();
            assert!(
                issue.ends_with(&format!("at line {line}, column 4")),
                "{plain}"
            );
        }

        let snippet = render(error, SnippetMode::Auto);
        for location in [":10000:4", ":10001:4"] {
            assert!(snippet.contains(location), "{snippet}");
        }
        assert!(snippet.contains("10000 | a: \"\""), "{snippet}");
        assert!(snippet.contains("10001 | b: \"\""), "{snippet}");

        let formatter = DefaultMessageFormatter.with_localizer(&Bracketed);
        let localized = error.render_with_options(serde_saphyr::render_options! {
            formatter: &formatter,
            line_offset: LINE_OFFSET,
            snippets: SnippetMode::Off,
        });
        let mut lines: Vec<_> = localized.lines().collect();
        lines.sort_unstable();
        assert_eq!(lines, ["check a [10000:4]", "check b [10001:4]"]);

        let custom = error.render_with_options(serde_saphyr::render_options! {
            formatter: &CustomMessage,
            line_offset: LINE_OFFSET,
            snippets: SnippetMode::Off,
        });
        assert_eq!(custom, "custom error");
        assert_eq!(error.to_string(), original);
    }

    #[track_caller]
    fn assert_multiple_documents(error: &Error) {
        let rendered = render(error, SnippetMode::Off);
        for line in [10001, 10002, 10004, 10005] {
            assert!(
                rendered.contains(&format!("at line {line}, column 4")),
                "{rendered}"
            );
        }
        let rendered = render(error, SnippetMode::Auto);
        for line in [10001, 10002, 10004, 10005] {
            assert!(rendered.contains(&format!(":{line}:4")), "{rendered}");
        }
    }

    #[cfg(feature = "garde")]
    #[test]
    fn garde_issues_and_multiple_documents_use_display_coordinates() {
        #[derive(Debug, Deserialize, garde::Validate)]
        struct Fields {
            #[garde(length(min = 1))]
            a: String,
            #[garde(length(min = 1))]
            b: String,
        }

        for error in [
            serde_saphyr::from_str_valid::<Fields>(YAML).unwrap_err(),
            serde_saphyr::from_reader_valid::<_, Fields>(YAML.as_bytes()).unwrap_err(),
        ] {
            assert_validation(&error);
        }
        let yaml = format!("---\n{YAML}---\n{YAML}");
        assert_multiple_documents(&serde_saphyr::from_multiple_valid::<Fields>(&yaml).unwrap_err());
    }

    #[cfg(feature = "validator")]
    #[test]
    fn validator_issues_and_multiple_documents_use_display_coordinates() {
        #[derive(Debug, Deserialize, validator::Validate)]
        struct Fields {
            #[validate(length(min = 1))]
            a: String,
            #[validate(length(min = 1))]
            b: String,
        }

        for error in [
            serde_saphyr::from_str_validate::<Fields>(YAML).unwrap_err(),
            serde_saphyr::from_reader_validate::<_, Fields>(YAML.as_bytes()).unwrap_err(),
        ] {
            assert_validation(&error);
        }
        let yaml = format!("---\n{YAML}---\n{YAML}");
        assert_multiple_documents(
            &serde_saphyr::from_multiple_validate::<Fields>(&yaml).unwrap_err(),
        );
    }
}
