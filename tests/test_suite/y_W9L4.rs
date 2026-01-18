// W9L4: Literal block scalar with more spaces in first line (invalid)
// Suite marks this as fail: true. Our test should ensure parser returns an error.

#[test]
fn yaml_w9l4_block_scalar_extra_indent_should_error() {
    // Encode significant spaces explicitly with \u{0020} to keep the test ASCII-only
    let y = "---\nblock scalar: |\n\u{0020}\u{0020}\u{0020}\u{0020}\u{0020}\n\u{0020}\u{0020}more spaces at the beginning\n\u{0020}\u{0020}are invalid\n";
    assert!(serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y).is_err());
}
