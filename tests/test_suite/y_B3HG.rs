// B3HG: Folded Scalar — expect "folded text\n"
#[test]
fn yaml_b3hg_folded_scalar() {
    let y = "--- >\n folded\n text\n\n\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse B3HG");
    assert_eq!(s, "folded text\n");
}
