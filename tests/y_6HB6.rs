use serde::Deserialize;

// 6HB6: Indentation Spaces — nested mapping and a flow-style-like list in dump
#[derive(Debug, Deserialize, PartialEq)]
struct Inner {
    #[serde(rename = "By one space")]
    by_one_space: String,
    #[serde(rename = "Flow style")]
    flow_style: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Root {
    #[serde(rename = "Not indented")]
    not_indented: Inner,
}

#[test]
fn yaml_6hb6_indentation_spaces() {
    // Use the simplified dump representation from the suite to avoid glyphs.
    let y = "Not indented:\n  By one space: |\n    By four\n      spaces\n  Flow style:\n  - By two\n  - Also by two\n  - Still by two\n";
    let d: Root = serde_saphyr::from_str(y).expect("failed to parse 6HB6");
    assert_eq!(d.not_indented.by_one_space, "By four\n  spaces\n");
    assert_eq!(d.not_indented.flow_style, vec![
        "By two".to_string(),
        "Also by two".to_string(),
        "Still by two".to_string(),
    ]);
}
