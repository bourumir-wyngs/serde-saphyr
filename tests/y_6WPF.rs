/// This test verifies YAML line folding inside double-quoted scalars.
///
/// According to the YAML 1.2 spec (§7.3.3.2):
/// - Line breaks in double-quoted scalars (not escaped with `\`) are converted to a single space.
/// - Leading and trailing spaces inside the quotes are preserved.
///
/// Example:
/// ```yaml
/// " foo
/// bar
/// baz "
/// ```
///
/// is parsed as the string:
/// ```text
///  foo bar baz
/// ```
///
/// This test ensures that `serde_saphyr` follows the spec by folding the
/// embedded newlines into spaces while retaining the outer whitespace.
#[test]
fn yaml_6wpf_flow_folding_in_double_quotes() {
    let y = "\" foo\nbar\nbaz \"\n";
    let s: String = serde_saphyr::from_str(y).expect("failed to parse 6WPF");
    assert_eq!(s, " foo bar baz ");
}
