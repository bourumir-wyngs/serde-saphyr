use indoc::indoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
}

#[test]
fn test_from_str_multi() {
    let yaml = indoc!("---\nx: 1\n---\nx: 2\n");
    let points: Vec<Point> = serde_saphyr::from_multiple(yaml).unwrap();
    assert_eq!(points, vec![Point { x: 1 }, Point { x: 2 }]);
}

#[test]
fn test_to_string_multi() {
    let points = vec![Point { x: 1 }, Point { x: 2 }];
    let out = serde_saphyr::to_string_multiple(&points).unwrap();
    assert_eq!(out, "x: 1\n---\nx: 2\n");
}
