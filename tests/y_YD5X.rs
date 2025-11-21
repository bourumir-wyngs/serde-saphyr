use serde::Deserialize;

// YD5X: Spec Example 2.5. Sequence of Sequences
// YAML is a sequence of 3 flow sequences. We'll deserialize into a vector of
// rows with mixed types using an untagged enum.

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Cell {
    S(String),
    I(i64),
    F(f64),
}

#[test]
fn yaml_yd5x_sequence_of_sequences() {
    let y = r#"- [name        , hr, avg  ]
- [Mark McGwire, 65, 0.278]
- [Sammy Sosa  , 63, 0.288]
"#;

    let rows: Vec<Vec<Cell>> = serde_saphyr::from_str(y).expect("failed to parse YD5X");
    assert_eq!(
        rows,
        vec![
            vec![Cell::S("name".into()), Cell::S("hr".into()), Cell::S("avg".into())],
            vec![
                Cell::S("Mark McGwire".into()),
                Cell::I(65),
                Cell::F(0.278),
            ],
            vec![Cell::S("Sammy Sosa".into()), Cell::I(63), Cell::F(0.288)],
        ]
    );
}
