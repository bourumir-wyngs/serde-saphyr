#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::{Error, SnippetMode};

fn render(error: &Error, source_name: Option<&str>) -> String {
    error.render_with_options(serde_saphyr::render_options! {
        source_name: source_name,
    })
}

#[test]
fn string_and_reader_source_names_are_deferred_and_combine_with_line_offsets() {
    for error in [
        serde_saphyr::from_str::<u8>("eighty\n").unwrap_err(),
        serde_saphyr::from_reader::<_, u8>("eighty\n".as_bytes()).unwrap_err(),
    ] {
        let baseline = error.to_string();
        let original_location = error.location().unwrap();
        let original_regions = match &error {
            Error::WithSnippet { regions, .. } => regions.clone(),
            _ => panic!("expected snippet"),
        };
        assert_eq!(render(&error, None), baseline);
        let name = String::from("article.md");
        let renamed = render(&error, Some(&name));
        assert!(renamed.contains("article.md:1:1"), "{renamed}");
        assert!(renamed.contains("1 | eighty"), "{renamed}");
        assert_eq!(render(&error, Some(&name)), renamed);

        let offset = error.render_with_options(serde_saphyr::render_options! {
            source_name: Some(&name),
            line_offset: 9_999,
        });
        assert!(offset.contains("article.md:10000:1"), "{offset}");
        assert!(offset.contains("10000 | eighty"), "{offset}");
        assert_eq!(error.location(), Some(original_location));
        assert_eq!(error.location().unwrap().span(), original_location.span());
        let Error::WithSnippet { regions, .. } = &error else {
            unreachable!()
        };
        assert_eq!(*regions, original_regions);
        assert_eq!(error.to_string(), baseline);
    }
}

#[test]
fn source_name_does_not_change_plain_rendering() {
    let error = serde_saphyr::from_str::<u8>("eighty").unwrap_err();
    for error in [&error, error.without_snippet()] {
        let plain = error.render_with_options(serde_saphyr::render_options! {
            snippets: SnippetMode::Off,
        });
        assert_eq!(
            error.render_with_options(serde_saphyr::render_options! {
                source_name: Some("article.md"),
                snippets: SnippetMode::Off,
            }),
            plain
        );
    }
    assert_eq!(
        render(error.without_snippet(), Some("article.md")),
        error.without_snippet().to_string()
    );
}

#[test]
fn names_escape_control_characters_and_an_empty_name_is_an_explicit_override() {
    let error = serde_saphyr::from_str::<u8>("eighty").unwrap_err();
    let rendered = render(&error, Some("part\n\u{1b}\u{2028}.md"));
    assert!(
        rendered.contains(r"part\n\u{1b}\u{2028}.md:1:1"),
        "{rendered}"
    );
    assert!(!rendered.contains('\u{1b}'));
    assert!(!rendered.contains('\u{2028}'));
    let rendered = render(&error, Some(""));
    assert!(rendered.contains("--> :1:1"), "{rendered}");
    assert!(!rendered.contains("input"), "{rendered}");
}

#[derive(Debug, Deserialize)]
struct Port {
    #[serde(rename = "port")]
    _port: u16,
}

#[test]
fn alias_use_and_distinct_failing_value_headers_use_the_override() {
    #[derive(Debug, Deserialize)]
    struct Config {
        #[serde(rename = "base")]
        _base: std::collections::BTreeMap<String, String>,
        #[serde(rename = "copy")]
        _copy: Port,
    }
    let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
    for error in [
        serde_saphyr::from_str::<Config>(yaml).unwrap_err(),
        serde_saphyr::from_reader::<_, Config>(yaml.as_bytes()).unwrap_err(),
    ] {
        let rendered = render(&error, Some("article.md"));
        assert!(rendered.contains("article.md:3:7"), "{rendered}");
        assert!(rendered.contains("article.md:2:9"), "{rendered}");
        assert!(rendered.contains("defined here"), "{rendered}");
        assert!(!rendered.contains("input>"), "{rendered}");
    }
}

#[cfg(feature = "include")]
#[test]
fn included_sources_keep_their_names_while_root_and_include_site_use_the_override() {
    use serde_saphyr::{IncludeRequest, IncludeResolveError, InputSource, ResolvedInclude};
    let options = || {
        serde_saphyr::options! {}.with_include_resolver(
            |_: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
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
        serde_saphyr::from_str_with_options::<Port>(yaml, options()).unwrap_err(),
        serde_saphyr::from_reader_with_options::<_, Port>(yaml.as_bytes(), options()).unwrap_err(),
    ] {
        let rendered = render(&error, Some("article.md"));
        assert!(rendered.contains("port.yaml:1:1"), "{rendered}");
        assert!(rendered.contains("article.md:1:16"), "{rendered}");
        assert!(rendered.contains("included from here"), "{rendered}");
    }
    let error =
        serde_saphyr::from_str_with_options::<Port>("port: eighty\n", options()).unwrap_err();
    let rendered = render(&error, Some("article.md"));
    assert!(rendered.contains("article.md:1:7"), "{rendered}");
}

#[cfg(any(feature = "garde", feature = "validator"))]
mod validation {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[cfg_attr(feature = "garde", derive(garde::Validate))]
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    struct Fields {
        #[cfg_attr(feature = "garde", garde(length(min = 1)))]
        #[cfg_attr(feature = "validator", validate(length(min = 1)))]
        a: String,
        #[cfg_attr(feature = "garde", garde(length(min = 1)))]
        #[cfg_attr(feature = "validator", validate(length(min = 1)))]
        b: String,
    }

    const YAML: &str = "a: \"\"\nb: \"\"\n";

    #[track_caller]
    fn assert_headers(error: &Error, lines: &[u64]) {
        let rendered = render(error, Some("article.md"));
        for line in lines {
            assert!(
                rendered.contains(&format!("article.md:{line}:4")),
                "{rendered}"
            );
        }
        assert!(!rendered.contains("input>"), "{rendered}");
    }

    #[cfg(feature = "garde")]
    #[test]
    fn garde_issue_headers_and_multiple_documents_use_the_override() {
        for error in [
            serde_saphyr::from_str_valid::<Fields>(YAML).unwrap_err(),
            serde_saphyr::from_reader_valid::<_, Fields>(YAML.as_bytes()).unwrap_err(),
        ] {
            assert_headers(&error, &[1, 2]);
        }
        let yaml = format!("---\n{YAML}---\n{YAML}");
        let error = serde_saphyr::from_multiple_valid::<Fields>(&yaml).unwrap_err();
        assert_headers(&error, &[2, 3, 5, 6]);
    }

    #[cfg(feature = "validator")]
    #[test]
    fn validator_issue_headers_and_multiple_documents_use_the_override() {
        for error in [
            serde_saphyr::from_str_validate::<Fields>(YAML).unwrap_err(),
            serde_saphyr::from_reader_validate::<_, Fields>(YAML.as_bytes()).unwrap_err(),
        ] {
            assert_headers(&error, &[1, 2]);
        }
        let yaml = format!("---\n{YAML}---\n{YAML}");
        let error = serde_saphyr::from_multiple_validate::<Fields>(&yaml).unwrap_err();
        assert_headers(&error, &[2, 3, 5, 6]);
    }
}
