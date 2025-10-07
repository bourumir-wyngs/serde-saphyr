use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn test_reader_deserialize() {
    let yaml = "x: 3\ny: 4\n";
    let reader = std::io::Cursor::new(yaml.as_bytes());
    let de = serde_saphyr::from_reader(reader);
    let p = Point::deserialize(de).unwrap();
    assert_eq!(p, Point { x: 3, y: 4 });
}

#[test]
fn test_large_reader_input() {
    let mut yaml = String::new();
    let mut i = 0usize;
    while yaml.len() < 64 * 1024 {
        yaml.push_str(&format!("k{0}: v{0}\n", i));
        i += 1;
    }

    let reader = std::io::Cursor::new(yaml.as_bytes());
    let _value: Value = serde_saphyr::from_reader(reader).unwrap();
}

#[test]
fn test_from_slice_map() {
    let yaml = b"x: 1\ny: 2\n";
    let m: HashMap<String, i32> = serde_saphyr::from_slice(yaml).unwrap();
    assert_eq!(m.get("x"), Some(&1));
}

#[test]
fn test_from_slice_multi_map() {
    let yaml = b"---\nx: 1\n---\nx: 2\n";
    let vals: Vec<HashMap<String, i32>> = serde_saphyr::from_slice_multiple(yaml).unwrap();
    assert_eq!(vals.len(), 2);
    assert_eq!(vals[0].get("x"), Some(&1));
    assert_eq!(vals[1].get("x"), Some(&2));
}
