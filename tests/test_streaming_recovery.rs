//! Tests for streaming reader error recovery.
//!
//! These tests verify that after a deserialization error in one document,
//! the streaming reader can recover and continue parsing subsequent documents.

use serde::Deserialize;
use std::io::Cursor;

#[derive(Debug, Deserialize, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

/// Test that after a deserialization error, the iterator recovers and continues
/// to the next document.
#[test]
fn streaming_reader_recovers_after_deserialization_error() {
    // First document has a type error (y is not a number)
    // Second document is valid
    let yaml = b"x: 1\ny: not_a_number\n---\nx: 10\ny: 20\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Point, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(
        results.len(),
        2,
        "Should have 2 results (1 error + 1 success)"
    );
    assert!(results[0].is_err(), "First document should fail");
    assert!(results[1].is_ok(), "Second document should succeed");
    assert_eq!(results[1].as_ref().unwrap(), &Point { x: 10, y: 20 });
}

/// Test recovery with multiple errors followed by valid documents.
#[test]
fn streaming_reader_recovers_multiple_errors() {
    // Doc 1: error (missing y)
    // Doc 2: error (wrong type for x)
    // Doc 3: valid
    // Doc 4: valid
    let yaml = b"x: 1\n---\nx: bad\ny: 2\n---\nx: 100\ny: 200\n---\nx: 300\ny: 400\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Point, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(results.len(), 4, "Should have 4 results");
    assert!(
        results[0].is_err(),
        "First document should fail (missing y)"
    );
    assert!(results[1].is_err(), "Second document should fail (bad x)");
    assert!(results[2].is_ok(), "Third document should succeed");
    assert_eq!(results[2].as_ref().unwrap(), &Point { x: 100, y: 200 });
    assert!(results[3].is_ok(), "Fourth document should succeed");
    assert_eq!(results[3].as_ref().unwrap(), &Point { x: 300, y: 400 });
}

/// Test that error in the last document doesn't cause issues.
#[test]
fn streaming_reader_error_in_last_document() {
    // Doc 1: valid
    // Doc 2: error (no more documents after)
    let yaml = b"x: 5\ny: 6\n---\nx: bad\ny: bad\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Point, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(results.len(), 2, "Should have 2 results");
    assert!(results[0].is_ok(), "First document should succeed");
    assert_eq!(results[0].as_ref().unwrap(), &Point { x: 5, y: 6 });
    assert!(results[1].is_err(), "Second document should fail");
}

/// Test recovery when error occurs mid-document with nested structure.
#[test]
fn streaming_reader_recovers_from_nested_error() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Nested {
        name: String,
        point: Point,
    }

    // Doc 1: error in nested structure
    // Doc 2: valid
    let yaml =
        b"name: first\npoint:\n  x: 1\n  y: bad\n---\nname: second\npoint:\n  x: 2\n  y: 3\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Nested, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(results.len(), 2, "Should have 2 results");
    assert!(results[0].is_err(), "First document should fail");
    assert!(results[1].is_ok(), "Second document should succeed");
    assert_eq!(
        results[1].as_ref().unwrap(),
        &Nested {
            name: "second".to_string(),
            point: Point { x: 2, y: 3 }
        }
    );
}

/// Test that all valid documents work without recovery needed.
#[test]
fn streaming_reader_all_valid_documents() {
    let yaml = b"x: 1\ny: 2\n---\nx: 3\ny: 4\n---\nx: 5\ny: 6\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Point, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(results.len(), 3, "Should have 3 results");
    assert!(results.iter().all(|r| r.is_ok()), "All should succeed");
    assert_eq!(results[0].as_ref().unwrap(), &Point { x: 1, y: 2 });
    assert_eq!(results[1].as_ref().unwrap(), &Point { x: 3, y: 4 });
    assert_eq!(results[2].as_ref().unwrap(), &Point { x: 5, y: 6 });
}

/// Test recovery with explicit document markers.
#[test]
fn streaming_reader_recovers_with_explicit_markers() {
    // Using explicit document start (---) and end (...) markers
    let yaml = b"---\nx: bad\ny: 1\n...\n---\nx: 10\ny: 20\n...\n";
    let mut reader = Cursor::new(&yaml[..]);

    let results: Vec<Result<Point, _>> = serde_saphyr::read(&mut reader).collect();

    assert_eq!(results.len(), 2, "Should have 2 results");
    assert!(results[0].is_err(), "First document should fail");
    assert!(results[1].is_ok(), "Second document should succeed");
    assert_eq!(results[1].as_ref().unwrap(), &Point { x: 10, y: 20 });
}
