#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::{FlowMap, FlowSeq, to_string};

#[test]
fn flow_map_nested_in_seq() {
    let v = vec![FlowMap(BTreeMap::from([("a", 1), ("b", 2)]))];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('{'), "expected flow map: {yaml}");
}

#[test]
fn flow_seq_inside_flow_seq() {
    let v = FlowSeq(vec![FlowSeq(vec![1i32, 2]), FlowSeq(vec![3i32, 4])]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("[["), "expected nested flow: {yaml}");
}

#[test]
fn flow_map_inside_flow_seq() {
    let v = FlowSeq(vec![FlowMap(BTreeMap::from([("a", 1i32)]))]);
    let yaml = to_string(&v).unwrap();
    assert!(
        yaml.contains("{a: 1}"),
        "expected flow map in flow seq: {yaml}"
    );
}

#[test]
fn seq_inside_flow_uses_flow_style() {
    // When in_flow > 0, take_flow_for_seq returns true (line 818)
    let v = FlowSeq(vec![vec![1i32, 2], vec![3i32, 4]]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('['), "expected flow: {yaml}");
}

#[test]
fn map_inside_flow_uses_flow_style() {
    // When in_flow > 0, take_flow_for_map returns true (line 828)
    let v = FlowSeq(vec![BTreeMap::from([("a", 1i32)])]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('{'), "expected flow map: {yaml}");
}

#[test]
fn flow_seq_at_top_level_ends_with_newline() {
    let v = FlowSeq(vec![1i32, 2, 3]);
    let yaml = to_string(&v).unwrap();
    assert!(yaml.trim() == "[1, 2, 3]", "yaml: {yaml}");
}

#[test]
fn flow_map_at_top_level_ends_with_newline() {
    let m = FlowMap(BTreeMap::from([("a", 1i32)]));
    let yaml = to_string(&m).unwrap();
    assert!(yaml.trim() == "{a: 1}", "yaml: {yaml}");
}

#[test]
fn flow_seq_deserialize_roundtrip() {
    let original = FlowSeq(vec![1i32, 2, 3]);
    let yaml = to_string(&original).unwrap();
    let back: FlowSeq<Vec<i32>> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, vec![1, 2, 3]);
}

#[test]
fn flow_map_deserialize_roundtrip() {
    let original = FlowMap(BTreeMap::from([("a".to_string(), 1i32)]));
    let yaml = to_string(&original).unwrap();
    let back: FlowMap<BTreeMap<String, i32>> = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, BTreeMap::from([("a".to_string(), 1)]));
}

#[test]
fn flow_seq_emits_brackets() {
    let yaml = to_string(&FlowSeq(vec![1, 2, 3])).unwrap();
    assert_eq!(yaml, "[1, 2, 3]\n");
}

#[test]
fn flow_map_emits_braces() {
    let mut m = BTreeMap::new();
    m.insert("a", 1);
    m.insert("b", 2);
    let yaml = to_string(&FlowMap(m)).unwrap();
    assert!(
        yaml.starts_with('{') && yaml.contains('}'),
        "expected flow map: {yaml}"
    );
}

#[test]
fn flow_map_as_struct_field() {
    #[derive(Serialize)]
    struct S {
        m: FlowMap<BTreeMap<String, i32>>,
    }
    let mut inner = BTreeMap::new();
    inner.insert("a".to_string(), 1);
    let s = S { m: FlowMap(inner) };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("m: {"), "expected inline flow map: {yaml}");
}

#[test]
fn flow_seq_as_struct_field() {
    #[derive(Serialize)]
    struct S {
        v: FlowSeq<Vec<i32>>,
    }
    let s = S {
        v: FlowSeq(vec![1, 2]),
    };
    let yaml = to_string(&s).unwrap();
    assert!(
        yaml.contains("v: [1, 2]"),
        "expected inline flow seq: {yaml}"
    );
}

#[test]
fn serialize_flow_seq() {
    use serde_saphyr::FlowSeq;
    #[derive(Serialize)]
    struct Doc {
        items: FlowSeq<Vec<i32>>,
    }
    let s = serde_saphyr::to_string(&Doc {
        items: FlowSeq(vec![1, 2, 3]),
    })
    .unwrap();
    assert!(s.contains('['));
}

#[test]
fn serialize_flow_map() {
    use serde_saphyr::FlowMap;
    use std::collections::BTreeMap;
    #[derive(Serialize)]
    struct Doc {
        data: FlowMap<BTreeMap<String, i32>>,
    }
    let mut m = BTreeMap::new();
    m.insert("a".into(), 1);
    let s = serde_saphyr::to_string(&Doc { data: FlowMap(m) }).unwrap();
    assert!(s.contains('{'));
}
