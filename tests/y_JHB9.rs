// JHB9: Two Documents in a Stream — two sequences of strings
// Assert we can parse both documents using from_multiple.

#[test]
fn yaml_jhb9_two_documents_in_stream() {
    let y = r#"# Ranking of 1998 home runs
---
- Mark McGwire
- Sammy Sosa
- Ken Griffey

# Team ranking
---
- Chicago Cubs
- St Louis Cardinals
"#;

    let docs: Vec<Vec<String>> = serde_saphyr::from_multiple(y).expect("failed to parse JHB9");
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0], vec![
        "Mark McGwire".to_string(),
        "Sammy Sosa".to_string(),
        "Ken Griffey".to_string(),
    ]);
    assert_eq!(docs[1], vec![
        "Chicago Cubs".to_string(),
        "St Louis Cardinals".to_string(),
    ]);
}
