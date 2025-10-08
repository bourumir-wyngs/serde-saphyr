use serde::Deserialize;

// 7BUB: Node for "Sammy Sosa" appears twice via anchor/alias
#[derive(Debug, Deserialize)]
struct Teams {
    hr: Vec<String>,
    rbi: Vec<String>,
}

#[test]
fn yaml_7bub_alias_occurrence_twice() {
    let y = "---\nhr:\n  - Mark McGwire\n  # Following node labeled SS\n  - &SS Sammy Sosa\nrbi:\n  - *SS # Subsequent occurrence\n  - Ken Griffey\n";
    let t: Teams = serde_saphyr::from_str(y).expect("failed to parse 7BUB");

    assert_eq!(t.hr, vec!["Mark McGwire".to_string(), "Sammy Sosa".to_string()]);
    assert_eq!(t.rbi, vec!["Sammy Sosa".to_string(), "Ken Griffey".to_string()]);
}