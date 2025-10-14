use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Shape {
    Circle { radius: f32 },
    Rectangle { width: f32, height: f32 },
}

#[test]
fn internally_tagged_enum_sequence_parses() {
    let yaml = r#"
- type: circle
  radius: 2.5
- type: rectangle
  width: 3
  height: 4
"#;

    let shapes: Vec<Shape> = serde_saphyr::from_str(yaml).expect("valid shapes YAML");

    assert_eq!(
        shapes,
        vec![
            Shape::Circle { radius: 2.5 },
            Shape::Rectangle { width: 3.0, height: 4.0 },
        ]
    );
}

#[test]
fn internally_tagged_unknown_tag_errors() {
    let yaml = r#"
- type: triangle
  base: 3
  height: 5
"#;
    let res: Result<Vec<Shape>, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "unknown tag should error");
}

#[test]
fn internally_tagged_missing_tag_errors() {
    let yaml = r#"
- radius: 1.0
"#;
    let res: Result<Vec<Shape>, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "missing tag field should error");
}

#[test]
fn internally_tagged_missing_required_field_errors() {
    // Rectangle requires width and height
    let yaml = r#"
- type: rectangle
  width: 10
"#;
    let res: Result<Vec<Shape>, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "missing required fields should error");
}

#[derive(Debug, Deserialize, PartialEq)]
struct Drawing {
    name: String,
    items: Vec<Shape>,
}

#[test]
fn internally_tagged_nested_in_struct_parses() {
    let yaml = r#"
name: sample
items:
  - type: circle
    radius: 1.25
  - type: rectangle
    width: 2
    height: 3
"#;

    let drawing: Drawing = serde_saphyr::from_str(yaml).expect("valid drawing YAML");

    assert_eq!(
        drawing,
        Drawing {
            name: "sample".to_string(),
            items: vec![
                Shape::Circle { radius: 1.25 },
                Shape::Rectangle { width: 2.0, height: 3.0 },
            ],
        }
    );
}
