use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn test_read_json_struct() {
    // JSON input
    let json = r#"{ "x": 1, "y": 2 }"#;

    // Parse JSON string using serde_yaml (JSON is valid YAML)
    let point: Point = serde_saphyr::from_str(json).unwrap();

    assert_eq!(point, Point { x: 1, y: 2 });
}

#[test]
fn test_read_json_array_of_structs() {
    // JSON input: an array of Point objects
    let json = r#"[ { "x": 1, "y": 2 }, { "x": 3, "y": 4 } ]"#;

    // Parse JSON (valid YAML) into Vec<Point>
    let points: Vec<Point> = serde_saphyr::from_str(json).unwrap();

    assert_eq!(points, vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 },]);

    // Serialize back to YAML
    let s = serde_saphyr::to_string(&points).unwrap();

    // YAML block style for arrays
    let expected = "\
- x: 1
  y: 2
- x: 3
  y: 4
";

    assert_eq!(s, expected);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Shape {
    name: String,
    vertices: Vec<Point>,
}

#[test]
fn test_read_json_nested_struct() {
    // JSON input: a map with a string and an array of Point objects
    let json = r#"
    {
        "name": "triangle",
        "vertices": [
            { "x": 0, "y": 0 },
            { "x": 1, "y": 0 },
            { "x": 0, "y": 1 }
        ]
    }
    "#;

    // Parse JSON into Shape
    let shape: Shape = serde_saphyr::from_str(json).unwrap();

    assert_eq!(
        shape,
        Shape {
            name: "triangle".to_string(),
            vertices: vec![
                Point { x: 0, y: 0 },
                Point { x: 1, y: 0 },
                Point { x: 0, y: 1 },
            ]
        }
    );
}
