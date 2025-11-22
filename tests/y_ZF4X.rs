use serde::Deserialize;
use std::collections::BTreeMap;

// ZF4X: Mapping of mappings (flow style across lines)

#[derive(Debug, Deserialize, PartialEq)]
struct Player {
    hr: i32,
    avg: f64,
}

#[test]
fn yaml_zf4x_mapping_of_mappings() {
    let y = r#"
Mark McGwire: {hr: 65, avg: 0.278}
Sammy Sosa: {
    hr: 63,
    avg: 0.288
  }
"#;

    let v: BTreeMap<String, Player> = serde_saphyr::from_str(y).expect("failed to parse ZF4X");
    let mm = v.get("Mark McGwire").expect("missing Mark McGwire");
    assert_eq!(mm.hr, 65);
    assert!((mm.avg - 0.278).abs() < 1e-9);
    let ss = v.get("Sammy Sosa").expect("missing Sammy Sosa");
    assert_eq!(ss.hr, 63);
    assert!((ss.avg - 0.288).abs() < 1e-9);
}
