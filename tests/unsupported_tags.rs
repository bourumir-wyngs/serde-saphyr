#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_saphyr::{Error, from_reader_with_options, from_str, from_str_with_options, options};
use std::io::Cursor;

fn strict_options() -> serde_saphyr::Options {
    options! { reject_unsupported_tags: true }
}

fn assert_unsupported_tag(error: &Error, expected: &str) {
    let error = error.without_snippet();
    assert!(
        matches!(error, Error::UnsupportedTag { tag, .. } if tag == expected),
        "unexpected error: {error:?}"
    );
    assert!(
        error
            .to_string()
            .starts_with(&format!("unsupported tag `{expected}`")),
        "unexpected message: {error}"
    );
    assert!(error.location().is_some());
}

#[test]
fn unsupported_tags_remain_permissive_by_default() {
    assert_eq!(from_str::<String>("!custom value").unwrap(), "value");
    assert_eq!(
        from_str::<Vec<String>>("!custom [one, two]").unwrap(),
        ["one", "two"]
    );
}

#[test]
fn strict_mode_rejects_unknown_tags_on_every_node_kind() {
    for yaml in ["!custom value", "!custom [value]", "!custom {key: value}"] {
        let error = from_str_with_options::<IgnoredAny>(yaml, strict_options()).unwrap_err();
        assert_unsupported_tag(&error, "!custom");
    }
}

#[test]
fn strict_mode_reports_the_source_tag_spelling() {
    for (yaml, expected) in [
        ("!!unknown value", "!!unknown"),
        (
            "!<tag:example.com,2026:unknown> value",
            "!<tag:example.com,2026:unknown>",
        ),
        (
            "%TAG !e! tag:example.com,2026:\n--- !e!unknown value",
            "!e!unknown",
        ),
    ] {
        let error = from_str_with_options::<IgnoredAny>(yaml, strict_options()).unwrap_err();
        assert_unsupported_tag(&error, expected);
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KnownOnly {
    known: u32,
}

#[test]
fn strict_mode_rejects_unknown_tags_inside_ignored_fields() {
    let error = from_str_with_options::<KnownOnly>(
        "known: 1\nignored: !custom {nested: value}\n",
        strict_options(),
    )
    .unwrap_err();
    assert_unsupported_tag(&error, "!custom");
}

#[test]
fn strict_mode_applies_to_reader_entrypoints() {
    let error =
        from_reader_with_options::<_, IgnoredAny>(Cursor::new("!custom value"), strict_options())
            .unwrap_err();
    assert_unsupported_tag(&error, "!custom");
}

#[test]
fn strict_mode_accepts_known_tags_including_merge_and_value() {
    assert_eq!(
        from_str_with_options::<String>("!!value value", strict_options()).unwrap(),
        "value"
    );

    let merged = from_str_with_options::<std::collections::BTreeMap<String, u32>>(
        "!!merge <<: {one: 1}\ntwo: 2\n",
        strict_options(),
    )
    .unwrap();
    assert_eq!(merged.get("one"), Some(&1));
    assert_eq!(merged.get("two"), Some(&2));
}

#[test]
fn strict_mode_rejects_custom_enum_tags() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    enum Tagged {
        Value(String),
    }

    let error = from_str_with_options::<Tagged>("!Value payload", strict_options()).unwrap_err();
    assert_unsupported_tag(&error, "!Value");
}
