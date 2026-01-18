use serde::Deserialize;

// 4ZYM: Spec Example 6.4. Line Prefixes
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    plain: String,
    quoted: String,
    block: String,
}

#[test]
fn yaml_4zym_line_prefixes() {
    let y = "plain: text\n  lines\nquoted: \"text\n  \tlines\"\nblock: |\n  text\n   \tlines\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse 4ZYM");
    assert_eq!(d.plain, "text lines");
    assert_eq!(d.quoted, "text lines");
    assert_eq!(d.block, "text\n \tlines\n");
}
