// TL85: Spec Example 6.8. Flow Folding (double-quoted scalar)
// Expected parsed value: " foo\nbar\nbaz "

#[test]
fn y_tl85_flow_folding_double_quoted() -> anyhow::Result<()> {
    let y = "\"\n  foo \n \n  bar\n\n  baz \n\"\n";

    let s: String = serde_saphyr::from_str(y)?;
    assert_eq!(s, " foo\nbar\nbaz ");
    Ok(())
}
