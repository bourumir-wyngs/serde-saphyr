#[test]
fn yaml_2lfx_reserved_directives_ignored_parse_scalar() {
    // Reserved directive should be ignored; the document value is the string "foo"
    let yaml = "%FOO  bar baz # Should be ignored\n# with a warning.\n---\n\"foo\"\n";

    let s: String = serde_saphyr::from_str(yaml).expect("failed to parse 2LFX scalar");
    assert_eq!(s, "foo");
}
