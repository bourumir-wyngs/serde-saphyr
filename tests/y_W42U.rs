use serde::Deserialize;

// W42U: Spec Example 8.15. Block Sequence Entry Types
// The YAML represents: [null, "block node\n", ["one", "two"], {one: "two"}]

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Entry {
    // null maps to None for Option<()>
    Null(Option<()>),
    Str(String),
    Seq(Vec<String>),
    Map { one: String },
}

#[test]
fn yaml_w42u_block_sequence_entry_types() {
    let y = r#"-
- |
 block node
- - one # Compact
  - two # sequence
- one: two # Compact mapping
"#;

    let v: Vec<Entry> = serde_saphyr::from_str(y).expect("failed to parse W42U");

    assert_eq!(v.len(), 4);
    // 1) null
    match &v[0] { Entry::Null(None) => {}, other => panic!("unexpected #0: {:?}", other) }
    // 2) block scalar with trailing newline
    match &v[1] { Entry::Str(s) => assert_eq!(s, "block node\n"), other => panic!("unexpected #1: {:?}", other) }
    // 3) sequence ["one", "two"]
    match &v[2] { Entry::Seq(xs) => assert_eq!(xs, &vec!["one".to_string(), "two".to_string()]), other => panic!("unexpected #2: {:?}", other) }
    // 4) mapping {one: "two"}
    match &v[3] { Entry::Map { one } => assert_eq!(one, "two"), other => panic!("unexpected #3: {:?}", other) }
}
