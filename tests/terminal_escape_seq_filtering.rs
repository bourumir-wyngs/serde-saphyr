#![cfg(all(feature = "serialize", feature = "deserialize"))]
//! Integration tests for terminal escape sequence filtering in error snippets.
//!
//! These tests verify that control characters (ASCII C0, DEL, UTF-8 C1) are properly
//! sanitized in error output to prevent terminal escape sequence injection.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_saphyr::{
    CroppedRegion, Error, Localizer, Location, MessageFormatter, SnippetMode, UserMessageFormatter,
    from_str,
};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TestStruct {
    field: i32, // Expect integer to trigger type errors
}

#[derive(Debug, Deserialize)]
enum TestEnum {
    Known,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct KnownFields {
    known: u32,
}

struct InjectingLocalizer;

impl Localizer for InjectingLocalizer {
    fn attach_location<'a>(&self, base: Cow<'a, str>, _loc: Location) -> Cow<'a, str> {
        Cow::Owned(format!("{base} localizer\n\u{1b}]0;owned\u{7}"))
    }

    fn snippet_location_prefix(&self, _loc: Location) -> String {
        "prefix\n\u{1b}[31m".to_owned()
    }
}

struct InjectingFormatter;

impl MessageFormatter for InjectingFormatter {
    fn localizer(&self) -> &dyn Localizer {
        &InjectingLocalizer
    }

    fn format_message<'a>(&self, _err: &'a Error) -> Cow<'a, str> {
        Cow::Borrowed("custom\nmessage\u{1b}[31m")
    }
}

/// Helper to extract the error message as a string for inspection.
fn error_to_string(err: &serde_saphyr::Error) -> String {
    format!("{}", err)
}

#[test]
fn yaml_derived_message_fields_are_escaped_at_the_render_boundary() {
    let raw_variant = "bad\u{1b}]0;owned\u{7}";
    let escaped_variant = r"bad\u{1b}]0;owned\u{7}";
    let error = from_str::<TestEnum>(r#""bad\e]0;owned\a""#).unwrap_err();

    assert!(
        matches!(
            error.without_snippet(),
            Error::SerdeUnknownVariant { variant, .. } if variant == raw_variant
        ),
        "the structured error must retain the decoded identifier: {error:?}"
    );

    for rendered in [
        error.to_string(),
        error.without_snippet().to_string(),
        error.render_with_formatter(&UserMessageFormatter),
    ] {
        assert!(rendered.contains(escaped_variant), "{rendered:?}");
        assert!(!rendered.contains(raw_variant), "{rendered:?}");
    }
}

#[test]
fn unknown_fields_and_duplicate_keys_use_the_same_render_boundary() {
    let raw_field = "forged\nrecord\u{1b}";
    let escaped_field = r"forged\nrecord\u{1b}";
    let field_error = from_str::<KnownFields>("known: 1\n\"forged\\nrecord\\e\": 2\n").unwrap_err();
    assert!(
        matches!(
            field_error.without_snippet(),
            Error::SerdeUnknownField { field, .. } if field == raw_field
        ),
        "the structured error must retain the decoded field: {field_error:?}"
    );

    let raw_key = "duplicate\u{1b}";
    let escaped_key = r"duplicate\u{1b}";
    let duplicate_error =
        from_str::<BTreeMap<String, u32>>("\"duplicate\\e\": 1\n\"duplicate\\e\": 2\n")
            .unwrap_err();
    assert!(
        matches!(
            duplicate_error.without_snippet(),
            Error::DuplicateMappingKey { key: Some(key), .. } if key == raw_key
        ),
        "the structured error must retain the decoded key: {duplicate_error:?}"
    );

    for (error, raw, escaped) in [
        (&field_error, raw_field, escaped_field),
        (&duplicate_error, raw_key, escaped_key),
    ] {
        for rendered in [
            error.to_string(),
            error.without_snippet().to_string(),
            error.render_with_formatter(&UserMessageFormatter),
        ] {
            assert!(rendered.contains(escaped), "{rendered:?}");
            assert!(!rendered.contains(raw), "{rendered:?}");
        }
    }
}

#[test]
fn custom_formatter_and_localizer_text_is_sanitized_before_layout() {
    let error = from_str::<TestStruct>("field: nope\n").unwrap_err();

    assert_eq!(
        InjectingFormatter.format_message(error.without_snippet()),
        "custom\nmessage\u{1b}[31m",
        "direct formatter calls are intentionally below the rendering boundary"
    );

    let plain = error.render_with_options(serde_saphyr::render_options! {
        formatter: &InjectingFormatter,
        snippets: SnippetMode::Off,
    });
    assert_eq!(
        plain,
        r"custom\nmessage\u{1b}[31m localizer\n\u{1b}]0;owned\u{7}"
    );

    let snippet = error.render_with_formatter(&InjectingFormatter);
    assert!(
        snippet.contains(r"prefix\n\u{1b}[31m: custom\nmessage\u{1b}[31m"),
        "{snippet:?}"
    );
    assert!(
        snippet.contains('\n'),
        "renderer-owned snippet layout should retain real newlines: {snippet:?}"
    );
    assert!(!snippet.contains("custom\nmessage\u{1b}"), "{snippet:?}");
}

#[test]
fn public_snippet_source_names_are_sanitized_at_render_time() {
    let source = "field: bad\n";
    let location = from_str::<TestStruct>(source)
        .unwrap_err()
        .location()
        .expect("type error should have a location");
    let error = Error::WithSnippet {
        regions: vec![CroppedRegion::new(
            source,
            "source\n\u{1b}]0;owned\u{7}.yaml",
            1,
            1,
            location,
        )],
        crop_radius: 2,
        error: Box::new(Error::Message {
            msg: "bad value".to_owned(),
            location,
        }),
    };

    let rendered = error.render();
    assert!(
        rendered.contains(r"source\n\u{1b}]0;owned\u{7}.yaml"),
        "{rendered:?}"
    );
    assert!(
        !rendered.contains("source\n\u{1b}]0;owned\u{7}.yaml"),
        "{rendered:?}"
    );
}

#[test]
fn test_ansi_escape_filtered_in_snippet() {
    // YAML with ANSI escape sequence (ESC = 0x1B = \x1B) in a string value
    // where we expect an integer, triggering a type error
    let yaml_with_escape = "field: \x1B[31mmalicious\x1B[0m";

    let err = from_str::<TestStruct>(yaml_with_escape).unwrap_err();
    let error_output = error_to_string(&err);

    // The ESC character (0x1B) should be replaced with space (0x20)
    // So the escape sequence should be broken/neutralized
    assert!(
        !error_output.contains("\x1B[31m"),
        "Error output should not contain ANSI escape sequences"
    );
    assert!(
        !error_output.contains("\x1B[0m"),
        "Error output should not contain ANSI reset sequences"
    );
}

#[test]
fn test_osc_escape_filtered_in_snippet() {
    // OSC (Operating System Command) escape: ESC ] (0x1B 0x5D)
    // Used for setting terminal title, etc.
    let yaml_with_osc = "field: \x1B]0;malicious title\x07";

    let err = from_str::<TestStruct>(yaml_with_osc).unwrap_err();
    let error_output = error_to_string(&err);

    // ESC should be replaced, breaking the OSC sequence
    assert!(
        !error_output.contains("\x1B]"),
        "Error output should not contain OSC escape sequences"
    );
}

#[test]
fn test_c1_control_filtered_in_snippet() {
    // UTF-8 encoded C1 control: CSI (Control Sequence Introducer) = U+009B = 0xC2 0x9B
    // This is an alternative way to introduce ANSI escapes
    let yaml_with_c1 = "field: \u{009B}31mmalicious";

    let err = from_str::<TestStruct>(yaml_with_c1).unwrap_err();
    let error_output = error_to_string(&err);

    // C1 control (0xC2 0x9B) should be replaced with NBSP (0xC2 0xA0)
    // The original C1 CSI should not appear
    assert!(
        !error_output.contains("\u{009B}"),
        "Error output should not contain C1 control characters"
    );
}

#[test]
fn test_del_character_filtered() {
    // DEL character (0x7F) can cause issues in some terminals
    let yaml_with_del = "field: test\x7Fvalue";

    let err = from_str::<TestStruct>(yaml_with_del).unwrap_err();
    let error_output = error_to_string(&err);

    // DEL should be replaced with space
    assert!(
        !error_output.contains('\x7F'),
        "Error output should not contain DEL character"
    );
}

#[test]
fn test_multiple_control_chars_filtered() {
    // Mix of various control characters
    let yaml_with_controls = "field: \x01\x02\x03test\x1B[31m\x7F";

    let err = from_str::<TestStruct>(yaml_with_controls).unwrap_err();
    let error_output = error_to_string(&err);

    // None of the control characters should appear
    assert!(!error_output.contains('\x01'), "SOH should be filtered");
    assert!(!error_output.contains('\x02'), "STX should be filtered");
    assert!(!error_output.contains('\x03'), "ETX should be filtered");
    assert!(!error_output.contains('\x1B'), "ESC should be filtered");
    assert!(!error_output.contains('\x7F'), "DEL should be filtered");
}

#[test]
fn test_newline_and_tab_preserved() {
    // \n and \t should be preserved as they're needed for snippet formatting
    // Use a map with an invalid field to trigger an error
    let yaml_with_whitespace = "field: 123\ninvalid_field:\n\tvalue";

    let err = from_str::<TestStruct>(yaml_with_whitespace).unwrap_err();
    let error_output = error_to_string(&err);

    // These should still be present (they're safe and needed)
    assert!(error_output.contains('\n'), "Newlines should be preserved");
    // Tab might or might not appear depending on the error location
}

#[test]
fn test_valid_utf8_preserved() {
    // Normal UTF-8 characters should pass through unchanged
    // Trigger a type error with unicode in the value
    let yaml_with_unicode = "field: Hello 世界 🌍";

    let err = from_str::<TestStruct>(yaml_with_unicode).unwrap_err();
    let error_output = error_to_string(&err);

    // Valid UTF-8 should be preserved in the error context
    // The error will show the value that couldn't be parsed as integer
    assert!(
        error_output.contains("Hello")
            || error_output.contains("世界")
            || error_output.contains("🌍"),
        "Valid UTF-8 should be preserved in error output"
    );
}

#[cfg(feature = "miette")]
#[test]
fn test_miette_integration_filters_escapes() {
    use serde_saphyr::miette::to_miette_report;

    // YAML with escape sequences that will cause a type error
    let yaml = "field: \x1B[31mmalicious\x1B[0m";

    let err = from_str::<TestStruct>(yaml).unwrap_err();
    let report = to_miette_report(&err, yaml, "test.yaml");

    // Use Display formatting to get the rendered report
    // The sanitization should prevent escape sequences from appearing in the source snippets
    let report_output = format!("{}", report);

    // The report should not contain the original escape sequences in the source snippet
    // (they should be replaced with spaces, breaking the ANSI codes)
    assert!(
        !report_output.contains("\x1B[31m"),
        "Miette report should not contain original ANSI escape sequences in source"
    );
}

#[cfg(feature = "miette")]
#[test]
fn test_miette_graphical_report_filters_escapes_from_snippet_regions() {
    use serde_saphyr::miette::to_miette_report;

    let yaml = "field: \x1B]0;malicious title\x07";

    let err = from_str::<TestStruct>(yaml).unwrap_err();
    let report = to_miette_report(&err, yaml, "test.yaml");
    let report_output = format!("{report:?}");

    assert!(
        !report_output.contains("\x1B]0;"),
        "Miette graphical report should not contain OSC escape sequences from snippets: {report_output:?}"
    );
    assert!(
        !report_output.contains('\x07'),
        "Miette graphical report should not contain BEL from OSC sequences: {report_output:?}"
    );
}

#[cfg(feature = "miette")]
#[test]
fn test_miette_filters_controls_from_message_fields() {
    use serde_saphyr::miette::to_miette_report;

    let raw_variant = "bad\u{1b}]0;owned\u{7}";
    let escaped_variant = r"bad\u{1b}]0;owned\u{7}";
    let yaml = r#""bad\e]0;owned\a""#;
    let error = from_str::<TestEnum>(yaml).unwrap_err();
    let raw_file = "test\n\u{1b}]0;owned\u{7}.yaml";
    let escaped_file = r"test\n\u{1b}]0;owned\u{7}.yaml";
    let report = to_miette_report(error.without_snippet(), yaml, raw_file);

    let displayed = format!("{report}");
    assert!(displayed.contains(escaped_variant), "{displayed:?}");
    assert!(!displayed.contains(raw_variant), "{displayed:?}");

    let rendered = format!("{report:?}");
    for (raw, escaped) in [(raw_variant, escaped_variant), (raw_file, escaped_file)] {
        assert!(rendered.contains(escaped), "{rendered:?}");
        assert!(!rendered.contains(raw), "{rendered:?}");
    }
}

#[test]
fn test_crlf_normalization_with_escapes() {
    // Test that CRLF normalization works together with escape filtering
    // Use CRLF line endings with escape sequences
    let yaml_with_crlf_and_escape = "field: \x1B[31mtest\x1B[0m\r\ninvalid: value";

    let err = from_str::<TestStruct>(yaml_with_crlf_and_escape).unwrap_err();
    let error_output = error_to_string(&err);

    // Escape sequences should be filtered
    assert!(
        !error_output.contains('\x1B'),
        "Escape sequences should be filtered even with CRLF"
    );
}
