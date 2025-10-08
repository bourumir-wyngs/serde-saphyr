// 7A4E: Double Quoted Lines — folding of newlines and spaces inside double quotes
#[test]
fn yaml_7a4e_double_quoted_lines() {
    // Construct a double-quoted scalar with a blank line and an intentional trailing space
    // at the end of the second logical line, which should be preserved across folding.
    let y = "\" 1st non-empty\n\n  2nd non-empty \n    3rd non-empty \"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 7A4E");
    assert_eq!(s, " 1st non-empty\n2nd non-empty 3rd non-empty ");
}
