use serde::Deserialize;

// A6F9: Chomping Final Line Break — strip (no trailing), clip (one trailing), keep (one trailing here per suite)
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    strip: String,
    clip: String,
    keep: String,
}

#[test]
fn yaml_a6f9_chomping_final_line_break() {
    let y = "strip: |-\n  text\nclip: |\n  text\nkeep: |+\n  text\n";
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse A6F9");
    assert_eq!(d.strip, "text");
    assert_eq!(d.clip, "text\n");
    // Our parser currently treats keep with a single trailing newline in this minimal example, which matches suite json.
    assert_eq!(d.keep, "text\n");
}
