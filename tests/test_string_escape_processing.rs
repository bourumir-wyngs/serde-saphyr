#[test]
fn test_double_quoted_strings_process_escape_sequences() {
    // Double-quoted YAML scalars process backslash escapes.
    let y = "\"line1\\nline2\\t\\\\\\\"\\u0041\"\n";
    let s: String = serde_saphyr::from_str(y).expect("Failed to parse double-quoted scalar");
    assert_eq!(s, "line1\nline2\t\\\"A");
}

#[test]
fn test_single_quoted_strings_do_not_process_backslash_escapes_but_double_quotes() {
    // Single-quoted YAML scalars do NOT process backslash escapes; the only escape is doubling '' -> '.
    let y = "'line1\\nline2\\t\\\\\\\"\\u0041 and it''s fine'\n";
    let s: String = serde_saphyr::from_str(y).expect("Failed to parse single-quoted scalar");
    assert_eq!(s, "line1\\nline2\\t\\\\\\\"\\u0041 and it's fine");
}
