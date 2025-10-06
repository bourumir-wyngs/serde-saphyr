use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[test]
fn config_example_compiles() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        name: String,
        enabled: bool,
        retries: i32,
    }

    let yaml_input = r#"
name: "My Application"
enabled: true
retries: 5
...
"#;

    let config: Config = serde_saphyr::from_str(yaml_input).expect("config YAML parses");

    assert_eq!(
        config,
        Config {
            name: "My Application".into(),
            enabled: true,
            retries: 5,
        },
    );
}

#[test]
fn nested_enum_example_compiles() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Move {
        by: f32,
        constraints: Vec<Constraint>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    enum Constraint {
        StayWithin { x: f32, y: f32, r: f32 },
        MaxSpeed { v: f32 },
    }

    let yaml = r#"
- by: 10.0
  constraints:
    - StayWithin:
        x: 0.0
        y: 0.0
        r: 5.0
    - StayWithin:
        x: 4.0
        y: 0.0
        r: 5.0
    - MaxSpeed:
        v: 3.5
"#;

    let robot_moves: Vec<Move> = serde_saphyr::from_str(yaml).expect("nested enum YAML parses");

    assert_eq!(robot_moves.len(), 1);
    assert!(matches!(
        &robot_moves[0].constraints[..],
        [
            Constraint::StayWithin { .. },
            Constraint::StayWithin { .. },
            Constraint::MaxSpeed { .. }
        ]
    ));
}

#[test]
fn composite_key_example_compiles() {
    #[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Transform {
        map: HashMap<Point, Point>,
    }

    let yaml = r#"
map:
  {x: 1, y: 2}: {x: 3, y: 4}
  {x: 5, y: 6}: {x: 7, y: 8}
"#;

    let transform: Transform = serde_saphyr::from_str(yaml).expect("composite key YAML parses");

    assert_eq!(transform.map.len(), 2);
}

#[test]
fn binary_scalar_example_compiles() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Blob {
        data: Vec<u8>,
    }

    let blob: Blob =
        serde_saphyr::from_str("data: !!binary aGVsbG8=").expect("binary scalar YAML parses");
    assert_eq!(blob.data, b"hello");
}


#[test]
fn readme_multiple_documents_stream_example_compiles() {
    #[derive(Debug, Deserialize, PartialEq)]
    enum Document {
        #[serde(rename = "person")]
        Person { name: String, age: u8 },
        #[serde(rename = "pet")]
        Pet { kind: String },
    }

    let input = r#"---
 person:
   name: Alice
   age: 30
---
 pet:
  kind: cat
---
 person:
   name: Bob
   age: 25
"#;

    let docs: Vec<Document> = serde_saphyr::from_multiple(input).expect("valid YAML stream");

    assert_eq!(
        docs,
        vec![
            Document::Person { name: "Alice".to_string(), age: 30 },
            Document::Pet { kind: "cat".to_string() },
            Document::Person { name: "Bob".to_string(), age: 25 },
        ]
    );
}
