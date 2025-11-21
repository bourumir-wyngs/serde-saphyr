use serde::Deserialize;

// XV9V: Spec Example 6.5. Empty Lines [1.3]
// Expect:
//   Folding: "Empty line\nas a line feed"
//   Chomping: "Clipped empty lines\n"
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    #[serde(rename = "Folding")]
    folding: String,
    #[serde(rename = "Chomping")]
    chomping: String,
}

#[test]
#[ignore]
fn yaml_xv9v_empty_lines_and_chomping() {
    let y = r#"Folding: "Empty line
as a line feed"
Chomping: |
  Clipped empty lines

"#;

    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse XV9V");
    assert_eq!(d.folding, "Empty line\nas a line feed");
    assert_eq!(d.chomping, "Clipped empty lines\n");
}
