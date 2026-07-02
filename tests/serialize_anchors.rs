#![cfg(all(feature = "serialize", feature = "deserialize"))]
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use indoc::indoc;
use serde::{Deserialize, Serialize};

// Bring helpers into scope
use serde_saphyr::{ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor, from_str, to_string};

#[derive(Clone, Serialize)]
struct Node {
    name: String,
    next: Option<RcAnchor<Node>>,     // allow cycles/shared via RcAnchor
    prev: Option<RcWeakAnchor<Node>>, // demonstrate weak back-reference
    unique: Option<RcAnchor<Node>>,   // an additional RcAnchor that may be unshared
}

#[derive(Clone, Serialize)]
struct NodeArc {
    name: String,
    next: Option<ArcAnchor<NodeArc>>, // allow cycles/shared via ArcAnchor
    prev: Option<ArcWeakAnchor<NodeArc>>, // demonstrate weak back-reference
    unique: Option<ArcAnchor<NodeArc>>, // an additional ArcAnchor that may be unshared
}

#[test]
fn rc_anchor_none_consumes_anchor_on_null_node() {
    #[derive(Debug, Deserialize, Serialize)]
    struct Doc {
        a: RcAnchor<Option<i32>>,
        x: i32,
        b: RcAnchor<Option<i32>>,
    }

    let shared = Rc::new(None);
    let doc = Doc {
        a: RcAnchor(shared.clone()),
        x: 5,
        b: RcAnchor(shared),
    };

    let yaml = to_string(&doc).expect("serialize shared None anchor");
    let expected = indoc! {r#"
        a: &a1 null
        x: 5
        b: *a1
    "#};
    assert_eq!(yaml, expected);

    let parsed: Doc = from_str(&yaml).expect("deserialize shared None anchor");
    assert_eq!(*parsed.a.0, None);
    assert_eq!(*parsed.b.0, None);
    assert!(
        Rc::ptr_eq(&parsed.a.0, &parsed.b.0),
        "alias should refer back to the anchored None node"
    );
}

#[test]
fn rc_anchor_unit_consumes_anchor_on_null_node() {
    #[derive(Debug, Deserialize, Serialize)]
    struct Doc {
        a: RcAnchor<()>,
        x: i32,
        b: RcAnchor<()>,
    }

    let shared = Rc::new(());
    let doc = Doc {
        a: RcAnchor(shared.clone()),
        x: 5,
        b: RcAnchor(shared),
    };

    let yaml = to_string(&doc).expect("serialize shared unit anchor");
    let expected = indoc! {r#"
        a: &a1 null
        x: 5
        b: *a1
    "#};
    assert_eq!(yaml, expected);

    let parsed: Doc = from_str(&yaml).expect("deserialize shared unit anchor");
    assert!(
        Rc::ptr_eq(&parsed.a.0, &parsed.b.0),
        "alias should refer back to the anchored unit node"
    );
}

#[test]
fn rc_anchor_enum_variants_anchor_the_variant_node() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Event {
        Newtype(i32),
        Tuple(i32, i32),
        Struct { value: i32 },
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct Doc {
        a: RcAnchor<Event>,
        x: i32,
        b: RcAnchor<Event>,
    }

    fn assert_round_trip(event: Event, expected: &str) {
        let shared = Rc::new(event);
        let doc = Doc {
            a: RcAnchor(shared.clone()),
            x: 5,
            b: RcAnchor(shared),
        };

        let yaml = to_string(&doc).expect("serialize shared enum variant anchor");
        assert_eq!(yaml, expected);

        let parsed: Doc = from_str(&yaml).expect("deserialize shared enum variant anchor");
        assert_eq!(*parsed.a.0, *parsed.b.0);
        assert!(
            Rc::ptr_eq(&parsed.a.0, &parsed.b.0),
            "alias should refer back to the anchored enum variant node"
        );
    }

    assert_round_trip(
        Event::Newtype(1),
        indoc! {r#"
            a: &a1
              Newtype: 1
            x: 5
            b: *a1
        "#},
    );
    assert_round_trip(
        Event::Tuple(1, 2),
        indoc! {r#"
            a: &a1
              Tuple:
                - 1
                - 2
            x: 5
            b: *a1
        "#},
    );
    assert_round_trip(
        Event::Struct { value: 1 },
        indoc! {r#"
            a: &a1
              Struct:
                value: 1
            x: 5
            b: *a1
        "#},
    );
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
    struct Wrap(ArcAnchor<NodeArc>);
    let shared = Arc::new(NodeArc {
        name: "shared".into(),
        next: None,
        prev: None,
        unique: None,
    });

    let mut map: BTreeMap<&str, Wrap> = BTreeMap::new();
    map.insert("a", Wrap(ArcAnchor(shared.clone())));
    map.insert("b", Wrap(ArcAnchor(shared)));

    let yaml = to_string(&map).expect("serialize ArcAnchor map");

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
    let strong = Arc::new(NodeArc {
        name: "strong".into(),
        next: None,
        prev: None,
        unique: None,
    });
    let weak = Arc::downgrade(&strong);
    let yaml = to_string(&ArcWeakAnchor(weak)).expect("serialize ArcWeakAnchor present");

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
        Holder {
            field: RcAnchor(outer.clone()),
        },
        Holder {
            field: RcAnchor(outer),
        },
    ];

    let yaml = to_string(&v).expect("serialize Holder with RcAnchor<Rc<String>>");

    // Avoid brittle formatting; ensure the important bits are present
    assert!(
        yaml.contains("&a1"),
        "Expected an anchor to be emitted, got: {}",
        yaml
    );
    assert!(
        yaml.contains("*a1"),
        "Expected an alias to be emitted, got: {}",
        yaml
    );
    assert!(
        yaml.contains("hello"),
        "Expected the string value to be present, got: {}",
        yaml
    );
}

#[test]
fn node_with_unique_unshared_and_present_weak() {
    // strong target for weak field
    let target = Rc::new(Node {
        name: "target".into(),
        next: None,
        prev: None,
        unique: None,
    });
    let weak_to_target = Rc::downgrade(&target);

    // parent has a weak ref to target and a unique unshared RcAnchor
    let parent = Node {
        name: "parent".into(),
        next: None,
        prev: Some(RcWeakAnchor(weak_to_target)),
        unique: Some(RcAnchor(Rc::new(Node {
            name: "unique".into(),
            next: None,
            prev: None,
            unique: None,
        }))),
    };

    let yaml = to_string(&parent).expect("serialize parent node with weak and unique");

    // We expect two anchors emitted (&a1 for target via weak, &a2 for unique), but no aliases
    // Accept either form depending on formatter: inline after colon (preferred) or on next line.
    let prev_ok = yaml.contains("prev: &a1") || yaml.contains("prev:\n  &a1");
    assert!(
        prev_ok,
        "prev should contain anchored target, got: {}",
        yaml
    );
    let unique_ok = yaml.contains("unique: &a2") || yaml.contains("unique:\n  &a2");
    assert!(
        unique_ok,
        "unique should contain its own anchor, got: {}",
        yaml
    );
    assert!(yaml.contains("name: target"));
    assert!(yaml.contains("name: unique"));
    assert!(
        !yaml.contains("*a2"),
        "unique is not shared, alias should not appear: {}",
        yaml
    );
}
