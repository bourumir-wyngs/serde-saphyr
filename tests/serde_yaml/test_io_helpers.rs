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
    let points: Vec<Point> = serde_saphyr::from_slice_multiple(multi).unwrap();
    assert_eq!(points, vec![Point { x: 1 }, Point { x: 2 }]);
}

#[test]
fn test_error_location() {
    let result: Result<Point, _> = serde_saphyr::from_str("@");
    let err = result.unwrap_err();
    let loc = err.location().expect("location missing");
    assert_eq!(loc.row, 1);
    assert_eq!(loc.column, 1);
}
