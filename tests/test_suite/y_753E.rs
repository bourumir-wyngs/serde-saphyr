// 753E: Block Scalar Strip (|-) → no trailing newline
#[test]
fn yaml_753e_block_scalar_strip() {
    let y = "--- |-\n ab\n\n\n...\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 753E");
    assert_eq!(s, "ab");
}
