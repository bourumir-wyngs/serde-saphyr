use serde_saphyr::from_str;
use serde_saphyr::{Error};

fn expect_location(err: &Error, line: u64, column: u64) {
    if let Some(loc) = err.location() {
        assert!(
            loc.column() == column && loc.line() == line,
            "Invalid location, expected line {line} column {column} reported {r_line} {r_column}",
            r_line = loc.line(),
            r_column = loc.column()
        );
        assert_eq!(loc.line(), line);
    } else {
        assert!(false, "Location was not provided");
    }
}

#[test]
fn parser_scan_error_carries_span() {
    let err = from_str::<Vec<String>>("[1, 2").expect_err("scan error expected");
    expect_location(&err, 2, 1);
    assert!(matches!(err, Error::Message { .. }));
}

#[test]
fn scalar_conversion_error_carries_span() {
    let err = from_str::<bool>("definitely").expect_err("bool parse error expected");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Message { .. }));
}

#[test]
fn unexpected_event_error_uses_event_location() {
    let err = from_str::<String>("- entry").expect_err("sequence cannot deserialize into string");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Unexpected { .. }));
}

#[test]
fn eof_error_reports_last_seen_position() {
    let err = from_str::<bool>("").expect_err("empty input should error");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Eof { .. }));
}

#[test]
fn parser_unknown_anchor_error_reports_location() {
    let err = from_str::<String>("*missing").expect_err("unknown anchor should error");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Message { .. }));
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
    assert!(matches!(err, Error::Message { .. }));
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
    assert!(matches!(err, Error::Unexpected { .. }));
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
    assert!(matches!(err, Error::Message { .. }));
}

// Additional diverse error cases

#[test]
fn scalar_conversion_error_with_indent_reports_column() {
    // Two leading spaces before an invalid bool should point to column 3.
    let err = from_str::<bool>(r#"  definitely"#).expect_err("bool parse error expected");
    expect_location(&err, 1, 3);
    assert!(matches!(err, Error::Message { .. }));
}

#[test]
fn unexpected_sequence_with_indent_reports_column() {
    // Two leading spaces before a sequence when a String is expected -> column 3.
    let err =
        from_str::<String>(r#"  - entry"#).expect_err("sequence cannot deserialize into string");
    expect_location(&err, 1, 3);
    assert!(matches!(err, Error::Unexpected { .. }));
}

#[test]
fn unexpected_mapping_when_string_expected() {
    // Mapping cannot be deserialized into a String.
    let err = from_str::<String>(r#"{k: v}"#).expect_err("mapping cannot deserialize into string");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Unexpected { .. }));
}

#[test]
fn unexpected_scalar_when_sequence_expected() {
    // Scalar cannot be deserialized into a Vec<_>.
    let err = from_str::<Vec<i32>>(r#"42"#).expect_err("scalar cannot deserialize into sequence");
    expect_location(&err, 1, 1);
    assert!(matches!(err, Error::Unexpected { .. }));
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
    assert!(matches!(err, Error::Eof { .. }));
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
    assert!(matches!(err, Error::Unexpected { .. }));
}
