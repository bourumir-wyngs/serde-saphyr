use serde::Deserialize;

// This test suite verifies that variable (polymorphic) documents can be
// deserialized using enums, both from a streaming reader iterator (read/read_with_options)
// and from a multi-document string (from_multiple/from_multiple_with_options).
//
// The shapes follow the patterns demonstrated in examples/polymorphism_tree_planting_robot.rs
// and README.md.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Tree {
    Oak,
    Acer,
    Birch,
}

fn default_birch() -> Tree { Tree::Birch }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Direction {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PlantArgs {
    #[serde(default = "default_birch")]
    tree: Tree,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
enum Command {
    #[serde(rename = "go")]
    Go { distance: usize },

    #[serde(rename = "turn")]
    Turn { direction: Direction },

    // Allow either `plant:` (null/empty) or `plant: { tree: ... }`
    #[serde(rename = "plant")]
    Plant(#[serde(default)] Option<PlantArgs>),

    #[serde(rename = "multistep")]
    MultiStep { steps: Vec<Command> },
}

#[test]
fn read_iterator_over_polymorphic_enum_documents() {
    // Five documents with varying enum variants. Includes a null-like `plant:` form.
    let yaml = r#"---
 go: { distance: 3 }
---
 plant:
---
 plant: { tree: Acer }
---
 turn: { direction: Left }
---
 multistep:
   steps:
     - go: { distance: 1 }
     - plant: { tree: Oak }
"#;

    let mut reader = std::io::Cursor::new(yaml.as_bytes());

    let iter = serde_saphyr::read::<_, Command>(&mut reader);
    let cmds: Vec<Command> = iter.map(|r| r.expect("failed to read enum doc"))
                                 .collect();

    assert_eq!(cmds.len(), 5);
    assert_eq!(cmds[0], Command::Go { distance: 3 });
    assert_eq!(cmds[1], Command::Plant(None)); // null/empty plant
    assert_eq!(cmds[2], Command::Plant(Some(PlantArgs { tree: Tree::Acer })));
    assert_eq!(cmds[3], Command::Turn { direction: Direction::Left });

    match &cmds[4] {
        Command::MultiStep { steps } => {
            assert_eq!(steps.len(), 2);
            assert_eq!(steps[0], Command::Go { distance: 1 });
            assert_eq!(steps[1], Command::Plant(Some(PlantArgs { tree: Tree::Oak })));
        }
        other => panic!("expected MultiStep, got {other:?}"),
    }
}

#[test]
fn from_multiple_string_with_polymorphic_enum_documents() {
    // Three documents: go, turn, plant(with default birch because args omitted)
    let yaml = r#"go: { distance: 42 }
---
turn: { direction: Right }
---
plant:
"#;

    let cmds: Vec<Command> = serde_saphyr::from_multiple(yaml).expect("from_multiple failed");
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0], Command::Go { distance: 42 });
    assert_eq!(cmds[1], Command::Turn { direction: Direction::Right });
    // Plant with no args yields Some(default) or None depending on our enum definition; here
    // we defined Plant(Option<PlantArgs>) so absent args map to None.
    assert_eq!(cmds[2], Command::Plant(None));
}
