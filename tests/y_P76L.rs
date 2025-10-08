// P76L: Secondary Tag Handle with !!int applied to non-integer content
// Expectation for our parser: treat as a plain string "1 - 3" (custom tags not mapped)

#[test]
fn yaml_p76l_secondary_tag_handle() {
    let y = r#"%TAG !! tag:example.com,2000:app/
---
!!int 1 - 3 # Interval, not integer
"#;
    // Try parsing as string; if the directive/tag is unsupported by parser, this may fail.
    let res: Result<String, _> = serde_saphyr::from_str(&y);
    match res {
        Ok(s) => assert_eq!(s, "1 - 3"),
        Err(e) => {
            // Some parsers may reject %TAG or secondary tag handles — mark as ignored if that happens.
            println!("IGNORED P76L: parser failed to handle %TAG/secondary tag handle: {e}");
            // Intentionally fail with ignore marker so CI can optionally skip; use cfg to allow local skip.
            // Use #[ignore] would be better, but we only know at runtime. So assert false with message to prompt investigation.
            // To comply with instructions, we'll keep this test active; maintainers can add #[ignore] if it consistently fails.
            panic!("P76L failed to parse; consider marking #[ignore] if parser limitation");
        }
    }
}
