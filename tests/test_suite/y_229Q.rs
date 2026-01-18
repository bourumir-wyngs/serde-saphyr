use serde::Deserialize;

// 229Q: Spec Example 2.4. Sequence of Mappings
#[derive(Debug, Deserialize, PartialEq)]
struct Player {
    name: String,
    hr: i32,
    avg: f32,
}

#[test]
fn yaml_229q_sequence_of_mappings() {
    let y = r#"-
  name: Mark McGwire
  hr:   65
  avg:  0.278
-
  name: Sammy Sosa
  hr:   63
  avg:  0.288
"#;
    let v: Vec<Player> = serde_saphyr::from_str(y).expect("failed to parse 229Q");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].name, "Mark McGwire");
    assert_eq!(v[0].hr, 65);
    assert!((v[0].avg - 0.278).abs() < 1e-6);
    assert_eq!(v[1].name, "Sammy Sosa");
    assert_eq!(v[1].hr, 63);
    assert!((v[1].avg - 0.288).abs() < 1e-6);
}
