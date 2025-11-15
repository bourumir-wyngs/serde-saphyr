// T26H: Spec Example 8.8. Literal Content [1.3]
#[test]
fn y_t26h_literal_content() {
    // Construct YAML exactly like in the suite but with real spaces instead of the visible-space glyphs.
    // The line with "# Comment" in the suite is outside the scalar content (treated as YAML comment),
    // so it is not included in the parsed value.
    let y = "--- |\n \n  \n  literal\n   \n  \n  text\n";

    let s: String = serde_saphyr::from_str(y).expect("failed to parse T26H literal block");
    assert_eq!(s, "\n\nliteral\n \n\ntext\n");
}
