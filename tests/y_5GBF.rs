use serde::Deserialize;
use serde_saphyr::Error;

// 5GBF: Empty lines and chomping behaviors
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    folding: String,
    folding_multi: String,
    chomping: String,
}

#[test]
fn yaml_5gbf_empty_lines_and_chomping() {
    // This is an error: invalid indentation in quoted scalar
    let y_wrong = r#"folding: "Empty line
as a line feed"
folding_multi: "More line feeds


and the end"

chomping: |
  Clipped empty lines

"#;

    let _d: Error= serde_saphyr::from_str::<Doc>(y_wrong).expect_err("This should fail");

    // Line break in quoted scalar must be folded in one space
    let y = r#"
folding: "Empty line
  as a line feed"

folding_multi: "Three line feeds


  and the end"

chomping: |
  Clipped empty lines

"#;

    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse");

    // Expect: Folding becomes "Empty line\nas a line feed",
    assert_eq!(d.folding, "Empty line as a line feed");

    assert_eq!(d.folding_multi, "Three line feeds\n\nand the end");

    // Chomping (with '|') keeps exactly one trailing '\n'.
    assert_eq!(d.chomping, "Clipped empty lines\n");
}
