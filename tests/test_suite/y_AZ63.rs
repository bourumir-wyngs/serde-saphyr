use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    one: Vec<i32>,
    four: i32,
}

// AZ63: Sequence with same indentation as parent mapping
#[test]
fn yaml_az63_sequence_same_indent_as_parent_mapping() {
    let y = "one:\n- 2\n- 3\nfour: 5\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse AZ63");
    assert_eq!(d.one, vec![2, 3]);
    assert_eq!(d.four, 5);
}
