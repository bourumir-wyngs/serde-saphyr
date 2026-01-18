// MJS9: Block Folding
#[test]
fn yaml_mjs9() {
    // YAML input for “Spec Example 6.7. Block Folding” (MJS9).
    // Important whitespace details:
    // - Line 2 has a trailing space after "foo".
    // - Line 3 consists of a single space.
    // - Line 4 has two spaces, then a TAB, then a space before "bar".
    // - There is a final newline after "baz".
    let yaml = concat!(">\n", "  foo \n", " \n", "  \t bar\n", "\n", "  baz\n",);
    let s: String = serde_saphyr::from_str(yaml).expect("failed to parse MJS9");
    assert_eq!(s, "foo \n\n\t bar\n\nbaz\n");
}
