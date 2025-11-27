use serde::Deserialize;
use serde_saphyr;

#[derive(Debug, Deserialize, PartialEq)]
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum SimpleTaggedEnum {
    Red,
    Blue,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Paint {
    color: Color,
}

#[test]
fn enum_unit_from_scalar() {
    let y = "color: Green\n";
    let paint: Paint = serde_saphyr::from_str(y).unwrap();
    assert_eq!(paint.color, Color::Green);
}

#[test]
fn tagged_enum_from_scalar_tag() {
    let y = "!!SimpleTaggedEnum RED";
    let value: SimpleTaggedEnum = serde_saphyr::from_str(y).unwrap();
    assert_eq!(value, SimpleTaggedEnum::Red);
}

#[test]
fn tagged_enum_rejects_nested_container() {
    let y = "!!SimpleTaggedEnum [RED]";
    let err = serde_saphyr::from_str::<SimpleTaggedEnum>(y).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("tagged enums must be scalars"));
}

#[derive(Debug, Deserialize, PartialEq)]
enum Status {
    Ok(u32),
    Err { code: i32 },
}

#[derive(Debug, Deserialize, PartialEq)]
struct Response {
    status: Status,
}

#[test]
fn enum_newtype_and_struct_variants() {
    let y = "status:\n  Err:\n    code: -7\n";
    let resp: Response = serde_saphyr::from_str(y).unwrap();
    assert_eq!(resp.status, Status::Err { code: -7 });

    let y2 = "status:\n  Ok: 200\n";
    let resp2: Response = serde_saphyr::from_str(y2).unwrap();
    assert_eq!(resp2.status, Status::Ok(200));
}

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

#[test]
fn nested_enum_deserialization() {
    let y = r#"- by: 10.0
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

    let moves: Vec<Move> = serde_saphyr::from_str(y).unwrap();

    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].by, 10.0);
    assert_eq!(
        moves[0].constraints,
        vec![
            Constraint::StayWithin {
                x: 0.0,
                y: 0.0,
                r: 5.0,
            },
            Constraint::StayWithin {
                x: 4.0,
                y: 0.0,
                r: 5.0,
            },
            Constraint::MaxSpeed { v: 3.5 },
        ],
    );
}
