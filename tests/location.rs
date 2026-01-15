use serde::Deserialize;
use serde_saphyr::from_str;
use serde_saphyr::{Error, Options};

fn unwrap_snippet(err: &Error) -> &Error {
    err.without_snippet()
}

fn expect_location(err: &Error, line: u64, column: u64) {
    if let Some(loc) = err.location() {
        assert_eq!(
            (loc.line(), loc.column()),
            (line, column),
            "Invalid location, expected line {line} column {column} reported {r_line} {r_column}",
            r_line = loc.line(),
            r_column = loc.column()
        );
    } else {
        panic!("Location was not provided");
    }
}

fn expect_span_offset(err: &Error, offset: usize) {
    if let Some(loc) = err.location() {
        assert_eq!(
            loc.span().offset(),
            offset,
            "Invalid span offset, expected {offset} reported {reported}",
            reported = loc.span().offset()
        );
    } else {
        panic!("Location was not provided");
    }
}

#[test]
fn parser_scan_error_carries_span() {
    let err = from_str::<Vec<String>>("[1, 2").expect_err("scan error expected");
    expect_location(&err, 2, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

#[test]
fn scalar_conversion_error_carries_span() {
    let err = from_str::<bool>("definitely").expect_err("bool parse error expected");
    expect_location(&err, 1, 1);
    expect_span_offset(&err, "definitely".find("definitely").unwrap());
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

#[test]
fn scalar_conversion_error_carries_span_offset_in_mapping_value() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct T {
        b: bool,
    }

    let yaml = "b: definitely\n";
    let err = from_str::<T>(yaml).expect_err("bool parse error expected");
    expect_location(&err, 1, 4);
    expect_span_offset(&err, yaml.find("definitely").unwrap());
}

#[test]
fn unexpected_event_error_uses_event_location() {
    let err = from_str::<String>("- entry").expect_err("sequence cannot deserialize into string");
    expect_location(&err, 1, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn eof_error_reports_last_seen_position() {
    let err = from_str::<bool>("").expect_err("empty input should error");
    expect_location(&err, 1, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Eof { .. }));
}

#[test]
fn parser_unknown_anchor_error_reports_location() {
    let err = from_str::<String>("*missing").expect_err("unknown anchor should error");
    expect_location(&err, 1, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

#[test]
fn scalar_conversion_error_carries_span_multiline() {
    // Value on the second line should report row 2, column 1 for the failing scalar.
    let err = from_str::<bool>(
        r#"
definitely"#,
    )
    .expect_err("bool parse error expected");
    expect_location(&err, 2, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

#[test]
fn unexpected_event_error_uses_event_location_multiline() {
    // Sequence start on the second line when a String is expected should point to row 2, col 1.
    let err = from_str::<String>(
        r#"
- entry"#,
    )
    .expect_err("sequence cannot deserialize into string");
    expect_location(&err, 2, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn parser_unknown_anchor_error_reports_location_multiline() {
    // Unknown alias on the second line should report its location.
    let err = from_str::<String>(
        r#"
*missing"#,
    )
    .expect_err("unknown anchor should error");
    expect_location(&err, 2, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

// Additional diverse error cases

#[test]
fn scalar_conversion_error_with_indent_reports_column() {
    // Two leading spaces before an invalid bool should point to column 3.
    let err = from_str::<bool>(r#"  definitely"#).expect_err("bool parse error expected");
    expect_location(&err, 1, 3);
    assert!(matches!(unwrap_snippet(&err), Error::Message { .. }));
}

#[test]
fn unexpected_sequence_with_indent_reports_column() {
    // Two leading spaces before a sequence when a String is expected -> column 3.
    let err =
        from_str::<String>(r#"  - entry"#).expect_err("sequence cannot deserialize into string");
    expect_location(&err, 1, 3);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn unexpected_mapping_when_string_expected() {
    // Mapping cannot be deserialized into a String.
    let err = from_str::<String>(r#"{k: v}"#).expect_err("mapping cannot deserialize into string");
    expect_location(&err, 1, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn unexpected_scalar_when_sequence_expected() {
    // Scalar cannot be deserialized into a Vec<_>.
    let err = from_str::<Vec<i32>>(r#"42"#).expect_err("scalar cannot deserialize into sequence");
    expect_location(&err, 1, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn eof_after_single_newline_reports_row2_col1() {
    // Empty second line after a newline: still EOF at row 2, col 1.
    let err = from_str::<bool>(
        r#"
"#,
    )
    .expect_err("empty input should error");
    expect_location(&err, 2, 1);
    assert!(matches!(unwrap_snippet(&err), Error::Eof { .. }));
}

#[test]
fn unexpected_mapping_on_second_line_with_indent() {
    // On second line with two spaces, mapping when String is expected -> row 2, col 3.
    let err = from_str::<String>(
        r#"
  k: 1"#,
    )
    .expect_err("mapping cannot deserialize into string");
    expect_location(&err, 2, 3);
    assert!(matches!(unwrap_snippet(&err), Error::Unexpected { .. }));
}

#[test]
fn error_with_snippet_renders_diagnostic_and_preserves_message() {
    let yaml = "*missing";
    // Render a plain error message (snippet disabled).
    let opts = Options {
        with_snippet: false,
        ..Default::default()
    };
    let err_plain = serde_saphyr::from_str_with_options::<String>(yaml, opts)
        .expect_err("unknown anchor should error");
    let plain = err_plain.to_string();

    // And compare to the default-rendered error (snippet enabled by default).
    let err = from_str::<String>(yaml).expect_err("unknown anchor should error");
    let rendered = err.to_string();

    assert!(
        rendered.contains(&plain),
        "rendered output must include the original message.\nplain: {plain}\nrendered: {rendered}"
    );
    assert!(
        rendered.contains("<input>:1:1"),
        "rendered output should include origin and coordinates.\nrendered: {rendered}"
    );
}

#[test]
fn with_snippet_enabled_by_default_in_from_str() {
    let yaml = "*missing";
    let err = from_str::<String>(yaml).expect_err("unknown anchor should error");
    let rendered = err.to_string();

    assert!(
        rendered.contains("<input>:1:1"),
        "default error rendering should include snippet origin/coordinates.\nrendered: {rendered}"
    );
}

#[test]
fn with_snippet_can_be_disabled_in_options() {
    let yaml = "*missing";

    let opts = Options {
        with_snippet: false,
        ..Default::default()
    };

    let err = serde_saphyr::from_str_with_options::<String>(yaml, opts)
        .expect_err("unknown anchor should error");
    let msg = err.to_string();

    assert!(
        !msg.contains("<input>:"),
        "snippet rendering should be disabled when Options::with_snippet is false.\nmsg: {msg}"
    );
    assert!(
        msg.contains("unknown anchor"),
        "message should still contain the original error.\nmsg: {msg}"
    );
}

#[test]
fn crop_radius_zero_disables_snippet_wrapping() {
    let yaml = "*missing";

    // Even when with_snippet is true, a radius of 0 means "no snippet".
    let opts = Options {
        crop_radius: 0,
        ..Default::default()
    };

    let err = serde_saphyr::from_str_with_options::<String>(yaml, opts)
        .expect_err("unknown anchor should error");
    let msg = err.to_string();

    assert!(
        !msg.contains("<input>:"),
        "snippet rendering should be disabled when Options::crop_radius is 0.\nmsg: {msg}"
    );
    assert!(
        msg.contains("unknown anchor"),
        "message should still contain the original error.\nmsg: {msg}"
    );
}

#[test]
fn with_snippet_only_contains_lines_near_error() {
    use std::fmt::Write as _;

    // We want the marker to be far outside the expected snippet window (about +-10 lines).
    // Put it near the top, and put the error near the bottom.
    let filler_lines = 100;

    let mut yaml = String::new();
    yaml.push_str("prefix: ok\n");
    yaml.push_str("marker_far_away: DO_NOT_INCLUDE\n");

    // Add enough valid mapping entries so the error is far away from the marker.
    for i in 0..filler_lines {
        writeln!(&mut yaml, "k{i}: v{i}").unwrap();
    }

    // Trigger an error at the end.
    yaml.push_str("bad: *missing\n");

    let err = serde_saphyr::from_str::<std::collections::HashMap<String, String>>(&yaml)
        .expect_err("unknown anchor should error");

    match err {
        Error::WithSnippet { text, .. } => {
            assert!(
                text.contains("<input>"),
                "expected snippet output header, got: {text}"
            );

            // Core requirement: snippet should NOT include content far away from the error.
            assert!(
                !text.contains("marker_far_away: DO_NOT_INCLUDE"),
                "snippet should not include far-away lines"
            );

            // If the contract is +-10 lines around the error, this should stay small.
            // Keep this bound generous for headers/formatting differences.
            assert!(text.lines().count() <= 60, "snippet has too many lines");
            assert!(text.len() < 10_000, "snippet output unexpectedly large");
        }
        other => panic!("expected WithSnippet wrapper, got: {other:?}"),
    }
}

#[test]
fn with_snippet_enabled_for_from_slice_with_options() {
    let yaml = "*missing";
    let err = serde_saphyr::from_slice_with_options::<String>(yaml.as_bytes(), Options::default())
        .expect_err("unknown anchor should error");
    let rendered = err.to_string();

    assert!(
        rendered.contains("<input>:1:1"),
        "from_slice_with_options should include snippet origin/coordinates by default.\nrendered: {rendered}"
    );
}
