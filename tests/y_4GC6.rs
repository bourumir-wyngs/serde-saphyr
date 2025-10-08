// 4GC6: Single quoted characters — doubled single quotes inside represent one quote
#[test]
fn yaml_4gc6_single_quoted_characters() {
    let y = "'here''s to \"quotes\"'\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 4GC6");
    assert_eq!(s, "here's to \"quotes\"");
}
