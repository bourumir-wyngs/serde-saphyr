// EX5H: Multiline scalar at top level (YAML 1.3 modified case)
// Expected combined content: "a b c d\ne"

#[test]
fn yaml_ex5h_multiline_scalar_top_level() {
    let y = r#"---
a
b
  c
d

 e
"#;
    // The original YAML suite uses visible space markers "␣" to illustrate spaces in line b.
    // Here we embed two literal spaces after 'b' to match the intended content.
    let y = y.replace('\u{001a}', " "); // replace placeholders with spaces to keep file readable

    let s: String = serde_saphyr::from_str(&y).expect("failed to parse EX5H");
    assert_eq!(s, "a b c d\ne");
}
