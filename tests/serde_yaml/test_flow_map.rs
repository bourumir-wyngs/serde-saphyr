use serde::Serialize;
use serde_saphyr::{FlowMap, to_string};
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Data {
    flow: FlowMap<BTreeMap<String, u32>>,
    block: BTreeMap<String, u32>,
}

#[test]
fn flow_mapping_renders_with_braces() {
    let mut m1: BTreeMap<String, u32> = BTreeMap::new();
    m1.insert("a".to_string(), 1);
    m1.insert("b".to_string(), 2);

    let mut m2: BTreeMap<String, u32> = BTreeMap::new();
    m2.insert("a".to_string(), 1);
    m2.insert("b".to_string(), 2);

    let data = Data {
        flow: FlowMap(m1),
        block: m2,
    };

    let yaml = to_string(&data).unwrap();
    assert_eq!(yaml, "flow: {a: 1, b: 2}\nblock:\n  a: 1\n  b: 2\n");
}

#[test]
fn test_flow_map_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut m: BTreeMap<String, i32> = BTreeMap::new();
    m.insert("x".to_string(), 10);
    m.insert("y".to_string(), 20);
    let f: FlowMap<BTreeMap<String, i32>> = FlowMap(m);

    let yaml = serde_saphyr::to_string(&f)?;
    let from_yaml: FlowMap<BTreeMap<String, i32>> = serde_saphyr::from_str(&yaml)?;

    assert_eq!(from_yaml, f, "Deserialized value should equal the original");

    assert!(
        yaml.trim_start().starts_with('{'),
        "Expected flow-style YAML mapping, got:\n{yaml}"
    );

    Ok(())
}
