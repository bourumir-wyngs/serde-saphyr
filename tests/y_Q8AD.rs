// Q8AD: Double Quoted Line Breaks [1.3]
#[test]
#[ignore]
fn yaml_q8ad_double_quoted_line_breaks() {
    let y = r#"---
"folded 
 to a space,
 
 to a line feed, or 	\
  \ ">>non-content"
"#;
    let s: String = serde_saphyr::from_str(&y).unwrap_or_else(|e| {
        println!("IGNORED? Q8AD: parser may not implement YAML 1.3 double-quoted folding rules: {e}");
        panic!("Q8AD failed to parse; consider marking #[ignore] if parser limitation");
    });
    assert_eq!(s, "folded to a space,\nto a line feed, or \t \tnon-content");
}
