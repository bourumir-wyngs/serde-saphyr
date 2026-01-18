use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct Node {
    x: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Doc {
    a: String,
    b: String,
    c: String,
    e: Vec<Node>,
    block: String,

    // key is the mapping is None to None
    // value: is sequence with one element, the scalar "seq2".
    thing: HashMap<Option<usize>, Vec<String>>,
}

#[test]
fn xw4d() -> anyhow::Result<()> {
    let yaml = r#"
a: "double
  quotes" # lala

b: plain
 value  # lala

c  : #lala
  d

thing:
  : # lala
    - #lala
      seq2

e: &node # lala
  - x: y

block: > # lala
  abcde
"#;

    let doc: Doc = serde_saphyr::from_str(yaml)?;

    // Assert parsed values
    assert_eq!(doc.a, "double quotes");
    assert_eq!(doc.b, "plain value");
    assert_eq!(doc.c, "d");

    // Folded block scalars with ">" should end with a trailing newline
    assert_eq!(doc.block, "abcde\n");

    // Sequence of one Node with x == "y"
    assert_eq!(doc.e.len(), 1);
    assert_eq!(doc.e[0].x, "y");

    // Mapping with a single key: None -> ["seq2"]
    assert_eq!(doc.thing.len(), 1);
    assert!(doc.thing.contains_key(&None));
    assert_eq!(doc.thing.get(&None), Some(&vec!["seq2".to_string()]));

    Ok(())
}
