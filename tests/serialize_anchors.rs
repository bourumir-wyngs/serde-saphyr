use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use indoc::indoc;
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
    #[derive(Clone, Serialize)]
    struct Wrap(RcAnchor<Node>);
    let shared = Rc::new(Node {
        name: "leaf".into(),
        next: None,
    });
    let seq = vec![Wrap(RcAnchor(shared.clone())), Wrap(RcAnchor(shared))];

    let yaml = to_string(&seq).expect("serialize RcAnchor sequence");
    println!("RC anchor seq:\n{}", yaml);

    let expected = indoc! {r#"
        - &a1
          name: leaf
          next: null
        - *a1
"#};
    assert_eq!(
        yaml, expected,
        "RC anchor seq YAML mismatch. Got:\n{}",
        yaml
    );
}

#[test]
fn arc_anchor_shared_in_map_has_repeated_anchor_and_values() {
    #[derive(Clone, Serialize)]
    struct Wrap(ArcAnchor<Node>);
    let shared = Arc::new(Node {
        name: "shared".into(),
        next: None,
    });

    let mut map: BTreeMap<&str, Wrap> = BTreeMap::new();
    map.insert("a", Wrap(ArcAnchor(shared.clone())));
    map.insert("b", Wrap(ArcAnchor(shared)));

    let yaml = to_string(&map).expect("serialize ArcAnchor map");
    println!("ARC anchor map:\n{}", yaml);

    let expected = indoc! {r#"
        a:&a1
        
          name: shared
          next: null
        b:*a1
    "#};
    assert_eq!(
        yaml, expected,
        "ARC anchor map YAML mismatch. Got:\n{}",
        yaml
    );
}

#[test]
fn rc_weak_anchor_present_serializes_under_anchor() {
    let strong = Rc::new(Node {
        name: "strong".into(),
        next: None,
    });
    let weak = Rc::downgrade(&strong);
    let yaml = to_string(&RcWeakAnchor(weak)).expect("serialize RcWeakAnchor present");
    println!("RC weak (present) as YAML:\n{}", yaml);

    let expected = indoc! {r#"
        &a1
        name: strong
        next: null
    "#};
    assert_eq!(
        yaml, expected,
        "RC weak (present) YAML mismatch. Got:\n{}",
        yaml
    );
}

#[test]
fn arc_weak_anchor_present_serializes_under_anchor() {
    let strong = Arc::new(Node {
        name: "strong".into(),
        next: None,
    });
    let weak = Arc::downgrade(&strong);
    let yaml = to_string(&ArcWeakAnchor(weak)).expect("serialize ArcWeakAnchor present");
    println!("ARC weak (present) as YAML:\n{}", yaml);

    let expected = indoc! {r#"
        &a1
        name: strong
        next: null
    "#};
    assert_eq!(
        yaml, expected,
        "ARC weak (present) YAML mismatch. Got:\n{}",
        yaml
    );
}

#[test]
fn rc_weak_anchor_dangling_serializes_as_null() {
    let weak = {
        let temp = Rc::new(Node {
            name: "temp".into(),
            next: None,
        });
        Rc::downgrade(&temp)
    }; // temp dropped -> weak now dangling
    let yaml = to_string(&RcWeakAnchor(weak)).expect("serialize RcWeakAnchor dangling");
    println!("RC weak (dangling) as YAML:\n{}", yaml);
    let expected = "null\n";
    assert_eq!(
        yaml, expected,
        "RC weak (dangling) YAML mismatch. Got:\n{}",
        yaml
    );
}
