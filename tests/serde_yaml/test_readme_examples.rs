use serde::{Deserialize, Serialize};

/// Test example 1 given in README
#[test]
fn example_main() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Config {
        name: String,
        enabled: bool,
        retries: i32,
    }

    let yaml_input = r#"
        name: "My Application"
        enabled: true
        retries: 5
    "#;

    let parsed: Config = serde_saphyr::from_str(yaml_input).expect("README example 1 should parse");
    assert_eq!(parsed.name, "My Application");
    assert!(parsed.enabled);
    assert_eq!(parsed.retries, 5);
}

/// Test example 2 given in README
#[test]
fn example_multi() -> anyhow::Result<()> {
    let configs = parse()?;
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].name, "My Application");
    assert!(configs[0].enabled);
    assert_eq!(configs[0].retries, 5);
    assert_eq!(configs[1].name, "My Debugger");
    assert!(!configs[1].enabled);
    assert_eq!(configs[1].retries, 4);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    name: String,
    enabled: bool,
    retries: i32,
}

fn parse() -> anyhow::Result<Vec<Config>> {
    let yaml_input = r#"
# Configure the application    
name: "My Application"
enabled: true
retries: 5
---
# Configure the debugger
name: "My Debugger"
enabled: false
retries: 4
"#;

    let configs = serde_saphyr::from_multiple(yaml_input)?;
    Ok(configs) // Ok on successful parsing or would be error on failure
}
/// Test nested enum example given in README
#[test]
fn example_nested() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum Outer {
        Inner(Inner),
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum Inner {
        Newtype(u8),
    }

    let yaml = indoc::indoc! {r#"
        Inner:
          Newtype: 0
    "#};

    let value: Outer = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(value, Outer::Inner(Inner::Newtype(0)));
}

#[derive(Deserialize, Serialize, Debug)]
#[allow(dead_code)]
#[derive(PartialEq)]
struct Move {
    by: f32,
    constraints: Vec<Constraint>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[allow(dead_code)]
enum Constraint {
    StayWithin { x: f32, y: f32, r: f32 },
    MaxSpeed { v: f32 },
}

#[test]
fn deserialize_robot_moves() {
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

    let robot_moves: Vec<Move> = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(robot_moves.len(), 1);
    assert_eq!(robot_moves[0].by, 10.0);
    assert_eq!(robot_moves[0].constraints.len(), 3);
}

#[test]
fn serialize_robot_moves() {
    let robot_moves: Vec<Move> = vec![
        Move {
            by: 1.0,
            constraints: vec![
                Constraint::StayWithin {
                    x: 0.0,
                    y: 0.0,
                    r: 5.0,
                },
                Constraint::MaxSpeed { v: 100.0 },
            ],
        },
        Move {
            by: 2.0,
            constraints: vec![Constraint::MaxSpeed { v: 10.0 }],
        },
    ];
    let yaml = serde_saphyr::to_string(&robot_moves).unwrap();
    let deserialized_robot_moves: Vec<Move> = serde_saphyr::from_str(&yaml)
        .unwrap_or_else(|_| panic!("Failed to deserialize robot moves, yaml:\n{yaml}"));
    assert_eq!(deserialized_robot_moves, robot_moves);
}
