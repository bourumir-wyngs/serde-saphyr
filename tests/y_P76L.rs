// P76L: Secondary Tag Handle with !!int applied to non-integer content
// Expectation for our parser: treat as a plain string "1 - 3" (custom tags not mapped)

#[test]
fn yaml_p76l_secondary_tag_handle() -> anyhow::Result<()> {
    let y = r#"%TAG !! tag:example.com,2000:app/
---
!!int 1 - 3 # Interval, not integer
"#;
    // Try parsing as string; if the directive/tag is unsupported by parser, this may fail.
    let s: String = serde_saphyr::from_str(y)?;
    assert_eq!(s, "1 - 3");
    Ok(())
}
