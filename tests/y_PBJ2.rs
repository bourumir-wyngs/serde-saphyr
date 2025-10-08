use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Leagues {
    american: Vec<String>,
    national: Vec<String>,
}

#[test]
fn yaml_pbj2_mapping_scalars_to_sequences() {
    let y = r#"american:
  - Boston Red Sox
  - Detroit Tigers
  - New York Yankees
national:
  - New York Mets
  - Chicago Cubs
  - Atlanta Braves
"#;
    let v: Leagues = serde_saphyr::from_str(y).expect("failed to parse PBJ2");
    assert_eq!(v.american, vec![
        "Boston Red Sox",
        "Detroit Tigers",
        "New York Yankees",
    ]);
    assert_eq!(v.national, vec![
        "New York Mets",
        "Chicago Cubs",
        "Atlanta Braves",
    ]);
}
