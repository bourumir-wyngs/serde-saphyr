// This example demonstrates how serde_saphyr serializes a map whose keys are
// non-scalar (struct) values. In YAML, such "complex keys" are emitted using the
// "?" query indicator for the key followed by a ":" for the value. For example:
//
//   map:
//     ? x: 1
//       y: 2
//     : x: 3
//       y: 4
//
// Depending on HashMap iteration order, the pairs may appear in either order.
// The example prints the YAML to stdout so you can inspect the formatting.
//
// How to run:
//   cargo run --example print_transform
//
// Related tests: see tests/serde_yaml/test_composite_keys.rs

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Point { x: i32, y: i32 }

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Transform { map: HashMap<Point, Point> }

fn main() {
    let mut map = HashMap::new();
    map.insert(Point { x: 1, y: 2 }, Point { x: 3, y: 4 });
    map.insert(Point { x: 5, y: 6 }, Point { x: 7, y: 8 });
    let transform = Transform { map };
    let yaml = serde_saphyr::to_string(&transform).unwrap();
    println!("{}", yaml);
}