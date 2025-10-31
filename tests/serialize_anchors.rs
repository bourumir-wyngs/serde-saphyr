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
    prev: Option<RcWeakAnchor<Node>>, // demonstrate weak back-reference
    unique: Option<RcAnchor<Node>>,   // an additional RcAnchor that may be unshared
}

#[test]
fn rc_anchor_shared_in_sequence_has_repeated_anchor_and_values() {
    #[derive(Clone, Serialize)]
    struct Wrap(RcAnchor<Node>);
    let shared = Rc::new(Node {
        name: "leaf".into(),
        next: None,
        prev: None,
        unique: None,
    });
    let seq = vec![Wrap(RcAnchor(shared.clone())), Wrap(RcAnchor(shared))];

    let yaml = to_string(&seq).expect("serialize RcAnchor sequence");
    println!("RC anchor seq:\n{}", yaml);

    let expected = indoc! {r#"
        - &a1
          name: leaf
          next: null
          prev: null
          unique: null
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
        prev: None,
        unique: None,
    });

    let mut map: BTreeMap<&str, Wrap> = BTreeMap::new();
    map.insert("a", Wrap(ArcAnchor(shared.clone())));
    map.insert("b", Wrap(ArcAnchor(shared)));

    let yaml = to_string(&map).expect("serialize ArcAnchor map");
    println!("ARC anchor map:\n{}", yaml);

    let expected = indoc! {r#"
        a: &a1
          name: shared
          next: null
          prev: null
          unique: null
        b: *a1
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
        prev: None,
        unique: None,
    });
    let weak = Rc::downgrade(&strong);
    let yaml = to_string(&RcWeakAnchor(weak)).expect("serialize RcWeakAnchor present");
    println!("RC weak (present) as YAML:\n{}", yaml);

    let expected = indoc! {r#"
        &a1
        name: strong
        next: null
        prev: null
        unique: null
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
        prev: None,
        unique: None,
    });
    let weak = Arc::downgrade(&strong);
    let yaml = to_string(&ArcWeakAnchor(weak)).expect("serialize ArcWeakAnchor present");
    println!("ARC weak (present) as YAML:\n{}", yaml);

    let expected = indoc! {r#"
        &a1
        name: strong
        next: null
        prev: null
        unique: null
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
            prev: None,
            unique: None,
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

#[test]
fn struct_with_rcanchor_of_rc_string_serializes_with_anchor_and_alias() {
    #[derive(Clone, Serialize)]
    struct Holder {
        field: RcAnchor<Rc<String>>, // desired shape: RcAnchor<Rc<String>>
    }

    // Build inner Rc<String>
    let inner: Rc<String> = Rc::new("hello".to_string());
    // Wrap it into an outer Rc so that RcAnchor<Rc<String>> holds Rc<Rc<String>>
    let outer: Rc<Rc<String>> = Rc::new(inner);

    // Create two holders that share the same outer Rc to trigger anchor + alias
    let v = vec![
        Holder { field: RcAnchor(outer.clone()) },
        Holder { field: RcAnchor(outer) },
    ];

    let yaml = to_string(&v).expect("serialize Holder with RcAnchor<Rc<String>>");
    println!("Holder YAML:\n{}", yaml);

    // Avoid brittle formatting; ensure the important bits are present
    assert!(yaml.contains("&a1"), "Expected an anchor to be emitted, got: {}", yaml);
    assert!(yaml.contains("*a1"), "Expected an alias to be emitted, got: {}", yaml);
    assert!(yaml.contains("hello"), "Expected the string value to be present, got: {}", yaml);
}

#[test]
fn node_with_unique_unshared_and_present_weak() {
    // strong target for weak field
    let target = Rc::new(Node { name: "target".into(), next: None, prev: None, unique: None });
    let weak_to_target = Rc::downgrade(&target);

    // parent has a weak ref to target and a unique unshared RcAnchor
    let parent = Node {
        name: "parent".into(),
        next: None,
        prev: Some(RcWeakAnchor(weak_to_target)),
        unique: Some(RcAnchor(Rc::new(Node { name: "unique".into(), next: None, prev: None, unique: None }))),
    };

    let yaml = to_string(&parent).expect("serialize parent node with weak and unique");
    println!("Parent with weak+unique YAML:\n{}", yaml);

    // We expect two anchors emitted (&a1 for target via weak, &a2 for unique), but no aliases
    // Accept either form depending on formatter: inline after colon (preferred) or on next line.
    let prev_ok = yaml.contains("prev: &a1") || yaml.contains("prev:\n  &a1");
    assert!(prev_ok, "prev should contain anchored target, got: {}", yaml);
    let unique_ok = yaml.contains("unique: &a2") || yaml.contains("unique:\n  &a2");
    assert!(unique_ok, "unique should contain its own anchor, got: {}", yaml);
    assert!(yaml.contains("name: target"));
    assert!(yaml.contains("name: unique"));
    assert!(!yaml.contains("*a2"), "unique is not shared, alias should not appear: {}", yaml);
}
