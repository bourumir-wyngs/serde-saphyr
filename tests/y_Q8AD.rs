// Q8AD: Double Quoted Line Breaks [1.3]
#[test]
fn yaml_q8ad_double_quoted_line_breaks() -> anyhow::Result<()>{
    let yaml: &str = "\
---\n\
\"folded \n\
to a space,\n\
 \n\
to a line feed, or \t\\\n\
 \\ \tnon-content\"\n";

    let expected: &str = "folded to a space,\nto a line feed, or \t \tnon-content";

    let s: String = serde_saphyr::from_str(&yaml)?;
    assert_eq!(s, expected);
    Ok(())
}
