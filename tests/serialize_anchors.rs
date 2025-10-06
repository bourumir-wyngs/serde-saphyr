use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use serde::Serialize;

// Bring helpers into scope
use serde_saphyr::{to_string, ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor};

#[derive(Clone, Serialize)]
struct Node {
    name: String,
    next: Option<RcAnchor<Node>>, // allow cycles/shared via RcAnchor
}

#[test]
fn rc_anchor_shared_in_sequence_has_repeated_anchor_and_values() {
    // Two references to the same Rc should produce the same anchor name twice
    // with full node definitions (current serializer behavior).
    let leaf = Rc::new(Node { name: "leaf".into(), next: None });
    let seq = vec![RcAnchor(leaf.clone()), RcAnchor(leaf)];
    let yaml = to_string(&seq).expect("serialize RcAnchor sequence");
    println!("RC anchor seq:\n{}", yaml);

    // Expect two list items and the same anchor name prefix twice.
    let dash_count = yaml.lines().filter(|l| l.trim_start().starts_with("- ")).count();
    assert_eq!(dash_count, 2, "expected two items, got: {}\n{}", dash_count, yaml);
    assert!(yaml.matches("&a").count() >= 2, "expected repeated anchor name, yaml: {}", yaml);
    assert_eq!(yaml.matches("name: leaf").count(), 2, "expected two 'name: leaf' entries. yaml: {}", yaml);
    assert_eq!(yaml.matches("next: null").count(), 2, "expected two 'next: null' entries. yaml: {}", yaml);
}

#[test]
fn arc_anchor_shared_in_map_has_repeated_anchor_and_values() {
    #[derive(Clone, Serialize)]
    struct Wrap(ArcAnchor<Node>);
    let shared = Arc::new(Node { name: "shared".into(), next: None });
    let mut map: BTreeMap<&str, Wrap> = BTreeMap::new();
    map.insert("a", Wrap(ArcAnchor(shared.clone())));
    map.insert("b", Wrap(ArcAnchor(shared)));
    let yaml = to_string(&map).expect("serialize ArcAnchor map");
    println!("ARC anchor map:\n{}", yaml);

    // Expect both keys and two anchor definitions with the same name prefix.
    assert!(yaml.contains("a:"), "missing key 'a:' in\n{}", yaml);
    assert!(yaml.contains("b:"), "missing key 'b:' in\n{}", yaml);
    assert!(yaml.matches("&a").count() >= 2, "expected repeated anchor name, yaml: {}", yaml);
    assert_eq!(yaml.matches("name: shared").count(), 2, "expected two 'name: shared' entries. yaml: {}", yaml);
    assert_eq!(yaml.matches("next: null").count(), 2, "expected two 'next: null' entries. yaml: {}", yaml);
}

#[test]
fn rc_weak_anchor_present_serializes_under_anchor() {
    let strong = Rc::new(Node { name: "strong".into(), next: None });
    let weak = Rc::downgrade(&strong);
    let yaml = to_string(&RcWeakAnchor(weak)).expect("serialize RcWeakAnchor present");
    println!("RC weak (present) as YAML:\n{}", yaml);

    assert!(yaml.starts_with("&a"), "expected anchor definition, yaml: {}", yaml);
    assert!(yaml.contains("name: strong"), "missing field, yaml: {}", yaml);
    assert!(yaml.contains("next: null"), "missing field, yaml: {}", yaml);
}

#[test]
fn arc_weak_anchor_present_serializes_under_anchor() {
    let strong = Arc::new(Node { name: "strong".into(), next: None });
    let weak = Arc::downgrade(&strong);
    let yaml = to_string(&ArcWeakAnchor(weak)).expect("serialize ArcWeakAnchor present");
    println!("ARC weak (present) as YAML:\n{}", yaml);

    assert!(yaml.starts_with("&a"), "expected anchor definition, yaml: {}", yaml);
    assert!(yaml.contains("name: strong"), "missing field, yaml: {}", yaml);
    assert!(yaml.contains("next: null"), "missing field, yaml: {}", yaml);
}

#[test]
fn rc_weak_anchor_dangling_serializes_as_null() {
    let weak = {
        let temp = Rc::new(Node { name: "temp".into(), next: None });
        Rc::downgrade(&temp)
    }; // temp dropped -> weak now dangling
    let yaml = to_string(&RcWeakAnchor(weak)).expect("serialize RcWeakAnchor dangling");
    println!("RC weak (dangling) as YAML:\n{}", yaml);
    let null_lines = yaml.lines().filter(|l| l.trim() == "null").count();
    assert!(null_lines >= 1, "expected at least one 'null' line, yaml: {}", yaml);
}
