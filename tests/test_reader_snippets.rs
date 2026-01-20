//! Integration tests for snippet rendering with reader-based parsing.
//!
//! These tests verify that when parsing YAML from a `std::io::Read` source,
//! error messages include helpful code snippets showing the error location.

use serde::Deserialize;
use std::io::Cursor;

#[derive(Debug, Deserialize, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

/// Test that a simple parse error from a reader includes a snippet.
#[test]
fn reader_error_includes_snippet() {
    let yaml = "x: 1\ny: not_a_number\n";
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<Point, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should mention the line and column
    assert!(
        err_str.contains("line 2"),
        "Error should mention line 2: {}",
        err_str
    );

    // The error should include the problematic YAML content
    assert!(
        err_str.contains("not_a_number"),
        "Error should show the problematic value: {}",
        err_str
    );
}

/// Test that errors at the beginning of the input show correct snippets.
#[test]
fn reader_error_at_start() {
    let yaml = "invalid: [unclosed\n";
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<Point, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // Should show the error location
    assert!(
        err_str.contains("line"),
        "Error should mention line: {}",
        err_str
    );
}

/// Test that type mismatch errors include snippets.
#[test]
fn reader_type_mismatch_error() {
    let yaml = "x: 1\ny: [1, 2, 3]\n"; // y should be i32, not a sequence
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<Point, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should be about type mismatch
    assert!(
        err_str.contains("line 2") || err_str.contains("expected"),
        "Error should indicate the problem: {}",
        err_str
    );
}

/// Test that missing field errors include snippets.
#[test]
fn reader_missing_field_error() {
    let yaml = "x: 1\n"; // missing 'y' field
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<Point, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should mention the missing field
    assert!(
        err_str.contains("y") || err_str.contains("missing"),
        "Error should mention missing field: {}",
        err_str
    );
}

/// Test with a larger input to verify the sliding window works.
#[test]
fn reader_large_input_error_at_end() {
    // Create a large valid YAML prefix followed by an error
    let mut yaml = String::new();
    for i in 0..1000 {
        yaml.push_str(&format!("key{}: value{}\n", i, i));
    }
    // Add an error at the end
    yaml.push_str("final: [unclosed\n");

    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<serde_json::Value, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should show context around the error location
    // Even though the input is large, we should see the problematic line
    assert!(
        err_str.contains("unclosed") || err_str.contains("line"),
        "Error should show context: {}",
        err_str
    );
}

/// Test that multi-line YAML errors show proper context.
#[test]
fn reader_multiline_context() {
    let yaml = r#"
name: test
items:
  - first
  - second
  - third: [unclosed
  - fourth
"#;
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<serde_json::Value, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // Should show context lines around the error
    assert!(
        err_str.contains("unclosed") || err_str.contains("third"),
        "Error should show the problematic line: {}",
        err_str
    );
}

/// Test that the snippet shows correct line numbers.
#[test]
fn reader_correct_line_numbers() {
    let yaml = "line1: ok\nline2: ok\nline3: [bad\n";
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<serde_json::Value, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should reference line 3 (or nearby)
    assert!(
        err_str.contains("line 3") || err_str.contains("line 4"),
        "Error should reference the correct line: {}",
        err_str
    );
}

/// Test that from_reader works correctly for valid input (no regression).
#[test]
fn reader_valid_input_works() {
    let yaml = "x: 10\ny: 20\n";
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<Point, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_ok());

    let point = result.unwrap();
    assert_eq!(point, Point { x: 10, y: 20 });
}

/// Test with UTF-8 content to ensure proper handling.
#[test]
fn reader_utf8_content() {
    let yaml = "name: 日本語\nvalue: [unclosed\n";
    let reader = Cursor::new(yaml.as_bytes());

    let result: Result<serde_json::Value, _> = serde_saphyr::from_reader(reader);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();

    // Should handle UTF-8 correctly
    assert!(
        err_str.contains("line"),
        "Error should show line info: {}",
        err_str
    );
}
