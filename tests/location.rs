use serde_saphyr::from_str;
use serde_saphyr::{Error, Location};

fn expect_location(err: &Error, expected: Location) {
    assert_eq!(
        err.location(),
        Some(expected),
        "expected location {:?}, got {:?}",
        expected,
        err.location()
    );
}

#[test]
fn parser_scan_error_carries_span() {
    let err = from_str::<Vec<String>>("[1, 2").expect_err("scan error expected");
    expect_location(&err, Location { row: 2, column: 1 });
    assert!(matches!(err, Error::Message { .. }));
}

#[test]
fn scalar_conversion_error_carries_span() {
    let err = from_str::<bool>("definitely").expect_err("bool parse error expected");
    expect_location(&err, Location { row: 1, column: 1 });
    assert!(matches!(err, Error::Message { .. }));
}

#[test]
fn unexpected_event_error_uses_event_location() {
    let err = from_str::<String>("- entry").expect_err("sequence cannot deserialize into string");
    expect_location(&err, Location { row: 1, column: 1 });
    assert!(matches!(err, Error::Unexpected { .. }));
}

#[test]
fn eof_error_reports_last_seen_position() {
    let err = from_str::<bool>("").expect_err("empty input should error");
    expect_location(&err, Location { row: 1, column: 1 });
    assert!(matches!(err, Error::Eof { .. }));
}

#[test]
fn parser_unknown_anchor_error_reports_location() {
    let err = from_str::<String>("*missing").expect_err("unknown anchor should error");
    expect_location(&err, Location { row: 1, column: 1 });
    assert!(matches!(err, Error::Message { .. }));
}
