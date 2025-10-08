use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Stats {
    hr: Vec<String>,
    rbi: Vec<String>,
}

// J9HZ: Single Document with Two Comments
#[test]
fn yaml_j9hz_single_document_with_two_comments() {
    let y = r#"---
hr:
  - Mark McGwire
  - Sammy Sosa
rbi:
  - Sammy Sosa
  - Ken Griffey
"#;
    let v: Stats = serde_saphyr::from_str(y).expect("failed to parse J9HZ");
    assert_eq!(v.hr, vec!["Mark McGwire".to_string(), "Sammy Sosa".to_string()]);
    assert_eq!(v.rbi, vec!["Sammy Sosa".to_string(), "Ken Griffey".to_string()]);
}
