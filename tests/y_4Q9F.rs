// 4Q9F: Folded Block Scalar [1.3]
#[test]
fn yaml_4q9f_folded_block_scalar() {
    let y = "--- >\n ab\n cd\n\n ef\n\n\n gh\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 4Q9F");
    assert_eq!(s, "ab cd\nef\n\ngh\n");
}
