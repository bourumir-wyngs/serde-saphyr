use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
}

#[test]
fn test_from_slice_and_multi() {
    let bytes = b"x: 1\n";
    let point: Point = serde_saphyr::from_slice(bytes).unwrap();
    assert_eq!(point, Point { x: 1 });

    let multi = b"---\nx: 1\n---\nx: 2\n";
    let points: Vec<Point> = serde_saphyr::from_slice_multi(multi).unwrap();
    assert_eq!(points, vec![Point { x: 1 }, Point { x: 2 }]);
}

#[test]
fn test_from_reader_multi() {
    let multi = b"---\nx: 1\n---\nx: 2\n".to_vec();
    let cursor = Cursor::new(multi);
    let points: Vec<Point> = serde_saphyr::from_reader_multi(cursor).unwrap();
    assert_eq!(points, vec![Point { x: 1 }, Point { x: 2 }]);
}

#[test]
fn test_to_writer_and_multi() {
    let mut buf = Vec::new();
    let point = Point { x: 1 };
    serde_saphyr::to_writer(&mut buf, &point).unwrap();
    assert_eq!(String::from_utf8(buf.clone()).unwrap(), "x: 1\n");

    buf.clear();
    let points = vec![Point { x: 1 }, Point { x: 2 }];
    serde_saphyr::to_writer_multi(&mut buf, &points).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "x: 1\n---\nx: 2\n");
}

#[test]
fn test_error_location() {
    let result: Result<Point, _> = serde_saphyr::from_str("@");
    let err = result.unwrap_err();
    let loc = err.location().expect("location missing");
    assert_eq!(loc.line(), 1);
    assert_eq!(loc.column(), 1);
}
