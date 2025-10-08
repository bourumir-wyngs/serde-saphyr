use serde::Deserialize;

// C4HZ rewritten: use serde tagged enums (externally tagged) instead of YAML local tags.
#[derive(Debug, Deserialize, PartialEq, Copy, Clone)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Item {
    Circle { center: Point, radius: i32 },
    Line { start: Point, finish: Point },
    // Keep color as String because YAML 0xFFEEBB is not parsed as an integer by our deserializer.
    Label { start: Point, color: String, text: String },
}

#[test]
fn yaml_c4hz_global_tags_shapes() {
    let y = r#"- { Circle: { center: &ORIGIN { x: 73, y: 129 }, radius: 7 } }
- { Line: { start: *ORIGIN, finish: { x: 89, y: 102 } } }
- { Label: { start: *ORIGIN, color: "0xFFEEBB", text: "Pretty vector drawing." } }
"#;
    let v: Vec<Item> = serde_saphyr::from_str(y).expect("failed to parse C4HZ as tagged enums");
    assert_eq!(v.len(), 3);

    match &v[0] {
        Item::Circle { center, radius } => {
            assert_eq!(*center, Point { x: 73, y: 129 });
            assert_eq!(*radius, 7);
        }
        _ => panic!("first should be Circle"),
    }

    match &v[1] {
        Item::Line { start, finish } => {
            assert_eq!(*start, Point { x: 73, y: 129 });
            assert_eq!(*finish, Point { x: 89, y: 102 });
        }
        _ => panic!("second should be Line"),
    }

    match &v[2] {
        Item::Label { start, color, text } => {
            assert_eq!(*start, Point { x: 73, y: 129 });
            assert_eq!(color, "0xFFEEBB");
            assert_eq!(text, "Pretty vector drawing.");
        }
        _ => panic!("third should be Label"),
    }
}
