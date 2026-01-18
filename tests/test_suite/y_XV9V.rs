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
fn yaml_xv9v_empty_lines_and_chomping_suite_exactish() {
    // Matches the YAML test-suite structure:
    // - Folding is a double-quoted scalar with an empty line inside
    // - Chomping is a literal block scalar
    // - After the block scalar: a line containing a single space, then final newline
    let y =
        "Folding:\n  \"Empty line\n\n  as a line feed\"\nChomping: |\n  Clipped empty lines\n \n";

    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse XV9V");
    assert_eq!(d.folding, "Empty line\nas a line feed");
    assert_eq!(d.chomping, "Clipped empty lines\n");
}
