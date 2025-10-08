// 7T8X: Folded Lines with final empty lines and comments
#[test]
fn yaml_7t8x_folded_lines_final_empty_lines() {
    let y = ">\n\n folded\n line\n\n next\n line\n   * bullet\n\n   * list\n   * lines\n\n last\n line\n\n# Comment\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 7T8X");
    assert_eq!(s, "\nfolded line\nnext line\n  * bullet\n\n  * list\n  * lines\n\nlast line\n");
}
