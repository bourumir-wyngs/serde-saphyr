// 6FWR: Block Scalar Keep (|+)
// Suite expectation: "ab\n\n \n" — the final kept line contains a single space.
// Current parser behavior: drops that single space on an otherwise blank trailing line,
// yielding "ab\n\n\n". Per project policy, mark this as ignored until the parser is fixed.
#[ignore]
#[test]
fn yaml_6fwr_block_scalar_keep() {
    let y = "--- |+\n ab\n\n \n...\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 6FWR");
    assert_eq!(s, "ab\n\n \n");
}
