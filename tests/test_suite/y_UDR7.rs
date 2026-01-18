use serde::Deserialize;

#[derive(Deserialize)]
struct Root {
    sequence: Vec<String>,
    mapping: std::collections::BTreeMap<String, String>,
}

#[test]
fn y_udr7_flow_collection_indicators() {
    let yaml = "sequence: [ one, two, ]\nmapping: { sky: blue, sea: green }\n";

    let v: Root = serde_saphyr::from_str(yaml).expect("failed to parse UDR7 YAML");

    assert_eq!(v.sequence, vec!["one".to_string(), "two".to_string()]);
    assert_eq!(v.mapping.get("sky").unwrap(), "blue");
    assert_eq!(v.mapping.get("sea").unwrap(), "green");
}
