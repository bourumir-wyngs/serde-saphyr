// 9TFX: Double Quoted Lines [1.3]
#[test]
fn yaml_9tfx_double_quoted_lines() {
    let y = "---\n\" 1st non-empty\n\n  2nd non-empty \n  3rd non-empty \"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 9TFX");
    assert_eq!(s, " 1st non-empty\n2nd non-empty 3rd non-empty ");
}
