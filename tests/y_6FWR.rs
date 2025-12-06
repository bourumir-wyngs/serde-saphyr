// 6FWR: Block Scalar Keep (|+)
// Suite expectation: "ab\n\n  \n" — the final kept line contains a single space.
// and discard another that is indentation.
#[test]
fn yaml_6fwr_block_scalar_keep() -> anyhow::Result<()>{
    // Two spaces
    let y = "--- |+\n ab\n\n  \n...\n";
    let s: String = serde_saphyr::from_str(y)?;
    // Only one left
    assert_eq!(s, "ab\n\n \n");
    Ok(())
}
