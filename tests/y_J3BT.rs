use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Program {
    quoted: String,
    block: String,
}

// J3BT: Tabs and Spaces —
// In quoted scalars, "\t" becomes an actual tab.
// In block scalars (|), "\t" stays as "\" + "t".
#[test]
fn yaml_j3bt_tabs_and_spaces() {
    let y = r#"quoted: "Quoted \t"
block: |
  void main() {
  \tprintf("Hello, world!\\n");
  }
"#;

    let v: Program = serde_saphyr::from_str(y).expect("failed to parse J3BT");

    // Quoted scalar: YAML interprets \t as a TAB character
    assert_eq!(v.quoted, "Quoted \t");

    // Block scalar: YAML preserves \t literally (two chars '\' and 't')
    assert_eq!(v.block, "void main() {\n\\tprintf(\"Hello, world!\\\\n\");\n}\n");
}
