use serde::Deserialize;

// 5GBF: Empty lines and chomping behaviors
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    #[serde(rename = "Folding")]
    folding: String,
    #[serde(rename = "Chomping")]
    chomping: String,
}

#[test]
#[ignore]
fn yaml_5gbf_empty_lines_and_chomping() {
    let y = "Folding: \"Empty line\nas a line feed\"\nChomping: |\n  Clipped empty lines\n\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse 5GBF");
    assert_eq!(d.folding, "Empty line\nas a line feed");
    assert_eq!(d.chomping, "Clipped empty lines\n");
}
