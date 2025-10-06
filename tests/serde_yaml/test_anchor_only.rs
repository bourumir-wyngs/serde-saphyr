use indoc::indoc;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Node {
    id: u32,
    name: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Root {
    first: Node,
    second: Node,
}

#[test]
fn test_anchor_struct_deserialization() {
    let yaml = indoc! {
        "
first: &node
  id: 1
  name: First
second: *node
"
    };

    let parsed: Root = serde_saphyr::from_str(yaml).expect("Failed to deserialize");
    let expected = Root {
        first: Node { id: 1, name: "First".into() },
        second: Node { id: 1, name: "First".into() },
    };

    assert_eq!(parsed, expected);
}

