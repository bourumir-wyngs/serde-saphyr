use indoc::indoc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Point {
    x: i32,
    xy: i32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Transform {
    map: HashMap<Point, Point>,
}

#[test]
fn test_deserialize_transform() {
    let yaml = indoc! {r#"
        map:
          {x: 1, xy: 2}: {x: 3, xy: 4}
          {x: 5, xy: 6}: {x: 7, xy: 8}
    "#};

    let mut map = HashMap::new();
    map.insert(Point { x: 1, xy: 2 }, Point { x: 3, xy: 4 });
    map.insert(Point { x: 5, xy: 6 }, Point { x: 7, xy: 8 });
    let expected = Transform { map };
    let actual: Transform = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn test_serialize_transform() {
    let mut map = HashMap::new();
    map.insert(Point { x: 1, xy: 2 }, Point { x: 3, xy: 4 });
    map.insert(Point { x: 5, xy: 6 }, Point { x: 7, xy: 8 });
    let transform = Transform { map };
    let yaml = serde_saphyr::to_string(&transform).unwrap();
    let expected_a = indoc! {
        "
        map:
          ? x: 1
            xy: 2
          : x: 3
            xy: 4
          ? x: 5
            xy: 6
          : x: 7
            xy: 8
        "
    };
    let expected_b = indoc! {
        "
        map:
          ? x: 5
            xy: 6
          : x: 7
            xy: 8
          ? x: 1
            xy: 2
          : x: 3
            xy: 4
        "
    };
    assert!(yaml == expected_a || yaml == expected_b);
}

#[test]
fn readme_main() {
    let yaml = r#"
  map:
      {x: 1, xy: 2}: {x: 3, xy: 4}
      {x: 5, xy: 6}: {x: 7, xy: 8}
"#;

    // Deserialize YAML into the Transform struct.
    let transform: Transform = serde_saphyr::from_str(yaml).unwrap();

    // Serializing will produce the same mapping (order may vary).
    let serialized = serde_saphyr::to_string(&transform).unwrap();
    println!("{}", serialized);
}
