use std::collections::HashMap;

// M5DY: Mapping between Sequences — complex keys are sequences; values are sequences of dates
// We represent the map as HashMap<Vec<String>, Vec<String>> and assert both entries.
#[test]
fn yaml_m5dy_mapping_between_sequences() {
    let y = r#"? - Detroit Tigers
  - Chicago cubs
:
  - 2001-07-23

? [ New York Yankees,
    Atlanta Braves ]
: [ 2001-07-02, 2001-08-12,
    2001-08-14 ]
"#;

    let v: HashMap<Vec<String>, Vec<String>> =
        serde_saphyr::from_str(y).expect("failed to parse M5DY");
    assert_eq!(v.len(), 2);

    let k1 = vec!["Detroit Tigers".to_string(), "Chicago cubs".to_string()];
    let d1 = v.get(&k1).expect("first key missing");
    assert_eq!(d1, &vec!["2001-07-23".to_string()]);

    let k2 = vec!["New York Yankees".to_string(), "Atlanta Braves".to_string()];
    let d2 = v.get(&k2).expect("second key missing");
    assert_eq!(
        d2,
        &vec![
            "2001-07-02".to_string(),
            "2001-08-12".to_string(),
            "2001-08-14".to_string()
        ]
    );
}
