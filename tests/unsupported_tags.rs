#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_saphyr::{Error, from_reader_with_options, from_str, from_str_with_options, options};
use std::collections::BTreeMap;
use std::io::Cursor;

fn strict_options() -> serde_saphyr::Options {
    options! { reject_unsupported_tags: true }
}

fn strict_robotics_options() -> serde_saphyr::Options {
    options! {
        reject_unsupported_tags: true,
        angle_conversions: true,
    }
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
fn strict_mode_accepts_merge_and_value_as_exact_scalar_mapping_keys() {
    let ordinary = from_str_with_options::<BTreeMap<String, String>>(
        "!!value =: library.dll\nversion: 1.2\n",
        strict_options(),
    )
    .unwrap();
    assert_eq!(ordinary.get("=").map(String::as_str), Some("library.dll"));
    assert_eq!(ordinary.get("version").map(String::as_str), Some("1.2"));

    let merged = from_str_with_options::<BTreeMap<String, u32>>(
        "!!merge <<: {one: 1}\ntwo: 2\n",
        strict_options(),
    )
    .unwrap();
    assert_eq!(merged.get("one"), Some(&1));
    assert_eq!(merged.get("two"), Some(&2));
}

#[test]
fn strict_mode_accepts_resolved_merge_and_value_key_tag_forms() {
    for yaml in [
        "!<tag:yaml.org,2002:value> '=': data",
        "%TAG !v! tag:yaml.org,2002:\n---\n!v!value =: data",
    ] {
        let value =
            from_str_with_options::<BTreeMap<String, String>>(yaml, strict_options()).unwrap();
        assert_eq!(value.get("=").map(String::as_str), Some("data"));
    }

    for yaml in [
        "!<tag:yaml.org,2002:merge> '<<': {one: 1}",
        "%TAG !m! tag:yaml.org,2002:\n---\n!m!merge <<: {one: 1}",
    ] {
        let value = from_str_with_options::<BTreeMap<String, u32>>(yaml, strict_options()).unwrap();
        assert_eq!(value.get("one"), Some(&1));
    }
}

#[test]
fn strict_mode_rejects_merge_and_value_outside_exact_scalar_mapping_keys() {
    for (yaml, expected) in [
        // Exact value-position examples from issue #180.
        ("x: !!merge foo", "!!merge"),
        ("x: !!value foo", "!!value"),
        // Correct special scalars, including quoted forms, are still invalid as values.
        ("x: !!merge <<", "!!merge"),
        ("x: !!value =", "!!value"),
        ("x: !!merge '<<'", "!!merge"),
        ("x: !!value '='", "!!value"),
        // Mapping keys with the wrong scalar content.
        ("!!merge foo: bar", "!!merge"),
        ("!!value foo: bar", "!!value"),
        // These tags are scalar-only, independently of mapping position.
        ("x: !!merge [foo]", "!!merge"),
        ("x: !!value {foo: bar}", "!!value"),
        ("!!merge [foo]", "!!merge"),
        ("!!value {foo: bar}", "!!value"),
        // A tagged scalar nested inside a complex key is not itself a mapping key.
        ("? [!!merge <<]\n: value", "!!merge"),
    ] {
        let error = from_str_with_options::<IgnoredAny>(yaml, strict_options()).unwrap_err();
        assert_unsupported_tag(&error, expected);
    }
}

#[test]
fn strict_mode_rejects_robotics_tags_without_angle_conversions() {
    for (yaml, expected_tag) in [("!degrees 180", "!degrees"), ("!radians 0.5", "!radians")] {
        let error = from_str_with_options::<f64>(yaml, strict_options()).unwrap_err();
        assert_unsupported_tag(&error, expected_tag);
    }
}

#[cfg(not(feature = "robotics"))]
#[test]
fn strict_mode_rejects_robotics_tags_without_compiled_support() {
    for (yaml, expected_tag) in [("!degrees 180", "!degrees"), ("!radians 0.5", "!radians")] {
        let error = from_str_with_options::<f64>(yaml, strict_robotics_options()).unwrap_err();
        assert_unsupported_tag(&error, expected_tag);
    }
}

#[cfg(feature = "robotics")]
#[test]
fn strict_mode_converts_robotics_tags_when_enabled() {
    let degrees = from_str_with_options::<f64>("!degrees 180", strict_robotics_options()).unwrap();
    assert!((degrees - std::f64::consts::PI).abs() < 1e-12);

    let radians = from_str_with_options::<f64>("!radians 0.5", strict_robotics_options()).unwrap();
    assert!((radians - 0.5).abs() < f64::EPSILON);
}

#[cfg(not(feature = "include"))]
#[test]
fn strict_mode_rejects_include_without_compiled_support() {
    let error =
        from_str_with_options::<IgnoredAny>("!include child.yaml", strict_options()).unwrap_err();
    assert_unsupported_tag(&error, "!include");
}

#[cfg(feature = "include")]
#[test]
fn strict_mode_rejects_include_without_configured_resolver() {
    let error =
        from_str_with_options::<IgnoredAny>("!include child.yaml", strict_options()).unwrap_err();
    assert_unsupported_tag(&error, "!include");
}

#[test]
fn strict_mode_rechecks_key_only_tags_when_aliases_are_replayed() {
    let error = from_str_with_options::<IgnoredAny>(
        "&tagged !!value =: definition\nmisused: *tagged\n",
        strict_options(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error.without_snippet(),
            Error::AliasError { msg, .. }
                if msg.contains("unsupported tag") && msg.contains("value")
        ),
        "unexpected error: {error:?}"
    );
    let locations = error
        .locations()
        .expect("alias error must report locations");
    assert_eq!(locations.reference_location.line(), 2);
    assert_eq!(locations.defined_location.line(), 1);
    assert_eq!(error.location().map(|location| location.line()), Some(2));
}

#[cfg(feature = "include")]
#[test]
fn strict_mode_tracks_key_context_across_resolved_includes() {
    let options = strict_options().with_include_resolver(|request| {
        Ok(serde_saphyr::ResolvedInclude::new(
            request.spec,
            "child.yaml",
            serde_saphyr::InputSource::from_string("!!value =: child-value\n".to_owned()),
        ))
    });

    let value: serde_json::Value = from_str_with_options(
        "included: !include child.yaml\n!!value =: root-value\n",
        options,
    )
    .unwrap();
    assert_eq!(value["included"]["="], "child-value");
    assert_eq!(value["="], "root-value");
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
