use serde::Deserialize;

// 5GBF: Empty lines and chomping behaviors
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    folding: String,
    folding_multi: String,
    chomping: String,
}

#[test]
fn yaml_5gbf_empty_lines_and_chomping() {
    // Strict mode rejects the under-indented quoted scalar continuations.
    let y_wrong = r#"folding: "Empty line
as a line feed"
folding_multi: "More line feeds


and the end"

chomping: |
  Clipped empty lines

"#;

    serde_saphyr::from_str_with_options::<Doc>(
        y_wrong,
        serde_saphyr::options! { strict_indentation: true },
    )
    .expect_err("strict mode rejects under-indented quoted continuations");

    let relaxed: Doc = serde_saphyr::from_str(y_wrong).expect("relaxed mode accepts continuations");
    assert_eq!(relaxed.folding, "Empty line as a line feed");
    assert_eq!(relaxed.folding_multi, "More line feeds\n\nand the end");
    assert_eq!(relaxed.chomping, "Clipped empty lines\n");

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
