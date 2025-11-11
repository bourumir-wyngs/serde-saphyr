// SSW6: Single quoted characters, YAML 1.3 modified
// Input: ---\n'here''s to "quotes"'\n
#[test]
fn y_ssw6_single_quoted_characters() {
    let y = "---\n'here''s to \"quotes\"'\n";
    let s: String = serde_saphyr::from_str(y).expect("Failed to parse single-quoted scalar");
    assert_eq!(s, "here's to \"quotes\"");
}
