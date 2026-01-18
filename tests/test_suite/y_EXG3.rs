// EXG3: Three dashes and content without space (YAML 1.3 modified)
// Expected combined string: "---word1 word2"

#[test]
fn yaml_exg3_three_dashes_and_content_without_space() {
    let y = r#"---
---word1
word2
"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse EXG3");
    assert_eq!(s, "---word1 word2");
}
